/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use app_units::{Au, MAX_AU, MIN_AU};
use base::id::ScrollTreeNodeId;
use euclid::{Box2D, Point2D};
use kurbo::Shape;
use malloc_size_of_derive::MallocSizeOf;
use paint_api::SerializableImageData;
use style::Zero;
use style::values::computed::basic_shape::{BasicShape, ClipPath};
use style::values::computed::position::Position;
use style::values::computed::{FillRule, LengthPercentage};
use style::values::generics::basic_shape::{
    GenericPolygon, GenericShapeRadius, PolygonCoord, ShapeBox, ShapeGeometryBox,
};
use style::values::generics::position::GenericPositionOrAuto;
use vello_cpu::kurbo;
use webrender_api::units::{
    LayoutPixel, LayoutPoint, LayoutRect, LayoutRectAu, LayoutSideOffsets, LayoutSize,
};
use webrender_api::{
    ImageDescriptor,
    BorderRadius, FillRule as WebrenderFillRule, ImageDescriptorFlags, ImageFormat, ImageKey,
    ImageMask,
};

use super::{BuilderForBoxFragment, compute_margin_box_radius, normalize_radii};
use crate::display_list::ClipTreeContext;

/// `clip-path: polygon(..)`s with more than this many vertices are ignored.
const CLIP_PATH_POLYGON_MAX_VERTICES: usize = 64;

/// An identifier for a clip used during StackingContextTree construction. This is a simple index in
/// a [`ClipStore`]s vector of clips.
#[derive(Clone, Copy, Debug, Eq, Hash, MallocSizeOf, PartialEq)]
pub(crate) struct ClipId(pub usize);

impl ClipId {
    /// Equivalent to [`ClipChainId::INVALID`]. This means "no clip."
    pub(crate) const INVALID: ClipId = ClipId(usize::MAX);
}

/// All the information needed to create a clip on a WebRender display list. These are created at
/// two times: during `StackingContextTree` creation and during WebRender display list construction.
/// Only the former are stored in a [`ClipStore`].
#[derive(Clone, MallocSizeOf)]
pub(crate) struct Clip {
    pub id: ClipId,
    pub area: ClipArea,
    pub parent_scroll_node_id: ScrollTreeNodeId,
    pub parent_clip_id: ClipId,
}

#[derive(Clone, MallocSizeOf)]
pub(crate) enum ClipArea {
    RoundedRect {
        radii: BorderRadius,
        rect: LayoutRect,
    },
    /// `clip-path: polygon(..)`
    Polygon {
        mask: ImageMask,
        fill_rule: WebrenderFillRule,
        vertices: Vec<LayoutPoint>,
    },
}

/// A simple vector of [`Clip`] that is built during `StackingContextTree` construction.
/// These are later turned into WebRender clips and clip chains during WebRender display
/// list construction.
#[derive(Clone, Default, MallocSizeOf)]
pub(crate) struct StackingContextTreeClipStore(pub Vec<Clip>);

impl StackingContextTreeClipStore {
    pub(super) fn get(&self, clip_id: ClipId) -> &Clip {
        &self.0[clip_id.0]
    }

    pub(crate) fn add(
        &mut self,
        area: ClipArea,
        parent_scroll_node_id: ScrollTreeNodeId,
        parent_clip_id: ClipId,
    ) -> ClipId {
        let id = ClipId(self.0.len());
        self.0.push(Clip {
            id,
            area,
            parent_scroll_node_id,
            parent_clip_id,
        });
        id
    }

    pub(super) fn add_for_clip_path(
        &mut self,
        clip_path: ClipPath,
        parent_scroll_node_id: ScrollTreeNodeId,
        parent_clip_chain_id: ClipId,
        fragment_builder: BuilderForBoxFragment,
        clip_context: &ClipTreeContext<'_>,
    ) -> Option<ClipId> {
        let geometry_box = match clip_path {
            ClipPath::Shape(_, ShapeGeometryBox::ShapeBox(shape_box)) => shape_box,
            ClipPath::Shape(_, ShapeGeometryBox::ElementDependent) => ShapeBox::BorderBox,
            ClipPath::Box(ShapeGeometryBox::ShapeBox(shape_box)) => shape_box,
            ClipPath::Box(ShapeGeometryBox::ElementDependent) => ShapeBox::BorderBox,
            _ => return None,
        };
        let layout_rect = match geometry_box {
            ShapeBox::BorderBox => fragment_builder.border_rect,
            ShapeBox::ContentBox => *fragment_builder.content_rect(),
            ShapeBox::PaddingBox => *fragment_builder.padding_rect(),
            ShapeBox::MarginBox => *fragment_builder.margin_rect(),
        };
        if let ClipPath::Shape(shape, _) = clip_path {
            match *shape {
                BasicShape::Circle(_) | BasicShape::Ellipse(_) | BasicShape::Rect(_) => self
                    .add_for_basic_shape(
                        *shape,
                        layout_rect,
                        parent_scroll_node_id,
                        parent_clip_chain_id,
                    ),
                BasicShape::Polygon(polygon) => self.add_for_polygon(
                    polygon,
                    layout_rect,
                    parent_scroll_node_id,
                    parent_clip_chain_id,
                    clip_context,
                ),
                BasicShape::PathOrShape(_) => None,
            }
        } else {
            let radii = match geometry_box {
                ShapeBox::MarginBox => compute_margin_box_radius(
                    fragment_builder.border_radius,
                    layout_rect.size(),
                    fragment_builder.fragment,
                ),
                _ => fragment_builder.border_radius,
            };
            let clip = self.add(
                ClipArea::RoundedRect {
                    radii,
                    rect: layout_rect,
                },
                parent_scroll_node_id,
                parent_clip_chain_id,
            );

            Some(clip)
        }
    }

    #[servo_tracing::instrument(name = "StackingContextClipStore::add_for_basic_shape", skip_all)]
    fn add_for_basic_shape(
        &mut self,
        shape: BasicShape,
        layout_box: LayoutRect,
        parent_scroll_node_id: ScrollTreeNodeId,
        parent_clip_chain_id: ClipId,
    ) -> Option<ClipId> {
        match shape {
            BasicShape::Rect(rect) => {
                let box_height = Au::from_f32_px(layout_box.height());
                let box_width = Au::from_f32_px(layout_box.width());
                let insets = LayoutSideOffsets::new(
                    rect.rect.0.to_used_value(box_height).to_f32_px(),
                    rect.rect.1.to_used_value(box_width).to_f32_px(),
                    rect.rect.2.to_used_value(box_height).to_f32_px(),
                    rect.rect.3.to_used_value(box_width).to_f32_px(),
                );

                // `inner_rect()` will cause an assertion failure if the insets are larger than the
                // rectangle dimension.
                let shape_rect = if insets.left + insets.right >= layout_box.width() ||
                    insets.top + insets.bottom > layout_box.height()
                {
                    LayoutRect::from_origin_and_size(layout_box.min, LayoutSize::zero())
                } else {
                    layout_box.to_rect().inner_rect(insets).to_box2d()
                };

                let corner = |corner: &style::values::computed::BorderCornerRadius| {
                    LayoutSize::new(
                        corner.0.width.0.to_used_value(box_width).to_f32_px(),
                        corner.0.height.0.to_used_value(box_height).to_f32_px(),
                    )
                };
                let mut radii = webrender_api::BorderRadius {
                    top_left: corner(&rect.round.top_left),
                    top_right: corner(&rect.round.top_right),
                    bottom_left: corner(&rect.round.bottom_left),
                    bottom_right: corner(&rect.round.bottom_right),
                };
                normalize_radii(&layout_box, &mut radii);
                Some(self.add(
                    ClipArea::RoundedRect {
                        radii,
                        rect: shape_rect,
                    },
                    parent_scroll_node_id,
                    parent_clip_chain_id,
                ))
            },
            BasicShape::Circle(circle) => {
                let center = match circle.position {
                    GenericPositionOrAuto::Position(position) => position,
                    GenericPositionOrAuto::Auto => Position::center(),
                };
                let anchor_x = center
                    .horizontal
                    .to_used_value(Au::from_f32_px(layout_box.width()));
                let anchor_y = center
                    .vertical
                    .to_used_value(Au::from_f32_px(layout_box.height()));
                let center = layout_box
                    .min
                    .add_size(&LayoutSize::new(anchor_x.to_f32_px(), anchor_y.to_f32_px()));

                let horizontal = compute_shape_radius(
                    center.x,
                    &circle.radius,
                    layout_box.min.x,
                    layout_box.max.x,
                );
                let vertical = compute_shape_radius(
                    center.y,
                    &circle.radius,
                    layout_box.min.y,
                    layout_box.max.y,
                );

                // If the value is `Length` then both values should be the same at this point.
                let radius = match circle.radius {
                    GenericShapeRadius::FarthestSide => horizontal.max(vertical),
                    GenericShapeRadius::ClosestSide => horizontal.min(vertical),
                    GenericShapeRadius::Length(_) => horizontal,
                };
                let radius = LayoutSize::new(radius, radius);
                let mut radii = webrender_api::BorderRadius {
                    top_left: radius,
                    top_right: radius,
                    bottom_left: radius,
                    bottom_right: radius,
                };
                let start = center.add_size(&-radius);
                let rect = LayoutRect::from_origin_and_size(start, radius * 2.);
                normalize_radii(&layout_box, &mut radii);
                Some(self.add(
                    ClipArea::RoundedRect { radii, rect },
                    parent_scroll_node_id,
                    parent_clip_chain_id,
                ))
            },
            BasicShape::Ellipse(ellipse) => {
                let center = match ellipse.position {
                    GenericPositionOrAuto::Position(position) => position,
                    GenericPositionOrAuto::Auto => Position::center(),
                };
                let anchor_x = center
                    .horizontal
                    .to_used_value(Au::from_f32_px(layout_box.width()));
                let anchor_y = center
                    .vertical
                    .to_used_value(Au::from_f32_px(layout_box.height()));
                let center = layout_box
                    .min
                    .add_size(&LayoutSize::new(anchor_x.to_f32_px(), anchor_y.to_f32_px()));

                let width = compute_shape_radius(
                    center.x,
                    &ellipse.semiaxis_x,
                    layout_box.min.x,
                    layout_box.max.x,
                );
                let height = compute_shape_radius(
                    center.y,
                    &ellipse.semiaxis_y,
                    layout_box.min.y,
                    layout_box.max.y,
                );

                let mut radii = webrender_api::BorderRadius {
                    top_left: LayoutSize::new(width, height),
                    top_right: LayoutSize::new(width, height),
                    bottom_left: LayoutSize::new(width, height),
                    bottom_right: LayoutSize::new(width, height),
                };
                let size = LayoutSize::new(width, height);
                let start = center.add_size(&-size);
                let rect = LayoutRect::from_origin_and_size(start, size * 2.);
                normalize_radii(&rect, &mut radii);
                Some(self.add(
                    ClipArea::RoundedRect { radii, rect },
                    parent_scroll_node_id,
                    parent_clip_chain_id,
                ))
            },
            _ => None,
        }
    }

    #[servo_tracing::instrument(name = "StackingContextClipStore::add_for_polygon", skip_all)]
    fn add_for_polygon(
        &mut self,
        polygon: GenericPolygon<LengthPercentage>,
        layout_box: LayoutRect,
        parent_scroll_node_id: ScrollTreeNodeId,
        parent_clip_chain_id: ClipId,
        clip_context: &ClipTreeContext<'_>,
    ) -> Option<ClipId> {
        println!("Add for polygon");
        if polygon.coordinates.len() > CLIP_PATH_POLYGON_MAX_VERTICES {
            log::warn!(
                "Ignoring \"clip-path: polygon()\" rule with {} vertices (more than {})",
                polygon.coordinates.len(),
                CLIP_PATH_POLYGON_MAX_VERTICES
            );
            return None;
        }

        // Construct a sequence of path operations that draw the polygon
        let Some(first_vertex) = polygon.coordinates.first() else {
            return None;
        };

        // Compute the used polygon coordinates. While we do this, we also compute
        // the bounding box of the polygon. If it's smaller than the layout_box then
        // we can save some space.
        let mut vertices = Vec::with_capacity(polygon.coordinates.len());
        let mut bounding_box = Box2D {
            min: Point2D::new(MAX_AU, MAX_AU),
            max: Point2D::new(MIN_AU, MIN_AU),
        };
        for coordinate in polygon.coordinates {
            let x = coordinate
                .0
                .to_used_value(Au::from_f32_px(layout_box.width()));
            let y = coordinate
                .1
                .to_used_value(Au::from_f32_px(layout_box.height()));

            bounding_box.min.x.min_assign(x);
            bounding_box.min.y.min_assign(y);
            bounding_box.max.x.max_assign(x);
            bounding_box.max.y.max_assign(y);

            vertices.push(Point2D::new(x, y));
        }

        // Compute path elements with offsets as needed
        let compute_offset = |coordinate: Point2D<Au, LayoutPixel>| -> kurbo::Point {
            let with_offset = coordinate - bounding_box.min;
            kurbo::Point::new(with_offset.x.to_f64_px(), with_offset.y.to_f64_px())
        };
        let mut path_elements: Vec<kurbo::PathEl> =
            Vec::with_capacity(vertices.len() + 1);

        path_elements.push(kurbo::PathEl::MoveTo(compute_offset(*vertices.first()?)));
        for vertex in vertices.iter().skip(1) {
            path_elements.push(kurbo::PathEl::LineTo(compute_offset(*vertex)));
        }
        path_elements.push(kurbo::PathEl::ClosePath);

        // Finally, draw the polygon to a bitmap that we can hand over to webrender
        let width = u16::try_from(bounding_box.width().to_nearest_px()).ok()?;
        let height = u16::try_from(bounding_box.height().to_nearest_px()).ok()?;
        let mut context = vello_cpu::RenderContext::new(width, height);

        // The vello docs state about tolerance (https://docs.rs/kurbo/0.13.0/kurbo/trait.Shape.html#tymethod.path_elements):
        // > For drawing as in UI elements, a value of 0.1 is appropriate, as it is unlikely to be visible to the eye.
        context.fill_path(&path_elements.into_path(0.1));
        context.flush();

        let mut target = vello_cpu::Pixmap::new(width, height);
        context.render_to_pixmap(&mut target);

        let descriptor = ImageDescriptor::new(
            width as i32,
            height as i32,
            ImageFormat::RGBA8,
            ImageDescriptorFlags::empty(),
        );
        let image_key = clip_context.image_cache.get_image_key()?;
        let data = SerializableImageData::Raw(ipc_channel::ipc::IpcSharedMemory::from_bytes(
            target.data_as_u8_slice(),
        ));
        clip_context
            .paint_api
            .add_image(image_key, descriptor, data, false);

        let fill_rule = match polygon.fill {
            FillRule::Evenodd => WebrenderFillRule::Evenodd,
            FillRule::Nonzero => WebrenderFillRule::Nonzero,
        };
        let mask = ImageMask {
            image: image_key,
            rect: Box2D {
                min: bounding_box.min.map(|coordinate| coordinate.to_f32_px()),
                max: bounding_box.max.map(|coordinate| coordinate.to_f32_px()),
            },
        };
        let webrender_vertices = vertices
            .into_iter()
            .map(|vertex| vertex.map(|coordinate| coordinate.to_f32_px()))
            .collect();
        let clip_id = self.add(
            ClipArea::Polygon {
                mask,
                vertices: webrender_vertices,
                fill_rule,
            },
            parent_scroll_node_id,
            parent_clip_chain_id,
        );
        Some(clip_id)

        // let bounds = layout_bounds.cast::<i32>().cast_unit();
        // let blob_commands = blob_commands
        //     .into_iter()
        //     .map(|data| BlobImageEntry { bounds, data })
        //     .collect::<Vec<_>>();
        // let compositor_api = &display_list.compositor_api;
        // let blob_data = Arc::new(bincode::serialize(&blob_commands).unwrap());
        // let absolute_bounds = layout_bounds.translate(layout_box.min.to_vector());
        // compositor_api.update_images(vec![ImageUpdate::AddBlobImage(
        //     blob_key,
        //     descriptor,
        //     layout_bounds.cast::<i32>().cast_unit(),
        //     blob_data,
        // )]);
        // let new_clip_id = display_list.wr.define_clip_image_mask(
        //     parent_scroll_node_id.spatial_id,
        //     ImageMask {
        //         image: blob_key.0,
        //         rect: absolute_bounds,
        //     },
        //     &vertices,
        //     match polygon.fill {
        //         FillRule::Evenodd => WrFillRule::Evenodd,
        //         FillRule::Nonzero => WrFillRule::Nonzero,
        //     },
        // );
        // Some(display_list.define_clip_chain(*parent_clip_chain_id, [new_clip_id]))
    }
}

fn compute_shape_radius(
    center: f32,
    radius: &GenericShapeRadius<LengthPercentage>,
    min_edge: f32,
    max_edge: f32,
) -> f32 {
    let distance_from_min_edge = (min_edge - center).abs();
    let distance_from_max_edge = (max_edge - center).abs();
    match radius {
        GenericShapeRadius::FarthestSide => distance_from_min_edge.max(distance_from_max_edge),
        GenericShapeRadius::ClosestSide => distance_from_min_edge.min(distance_from_max_edge),
        GenericShapeRadius::Length(length) => length
            .to_used_value(Au::from_f32_px(max_edge - min_edge))
            .to_f32_px(),
    }
}
