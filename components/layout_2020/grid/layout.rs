/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use app_units::Au;
use servo_arc::Arc;
use style::properties::ComputedValues;
use style::values::computed::{AlignContent, GridAutoFlow, LengthPercentage};
use style::values::generics::grid::{GenericTrackBreadth, GenericTrackListValue, GenericTrackSize};
use style::values::generics::length::{GenericSize, Size};
use style::values::specified::align::AlignFlags;
use style::values::specified::{ContentDistribution, GenericGridTemplateComponent};

use super::geom::{GridArea, GridCell, GridDimension};
use super::{GridFormattingContext, GridItemBox, OccupationGrid};
use crate::cell::ArcRefCell;
use crate::context::LayoutContext;
use crate::formatting_contexts::{Baselines, IndependentFormattingContext, IndependentLayout};
use crate::fragment_tree::Fragment;
use crate::geom::{AuOrAuto, LogicalRect, LogicalSides, LogicalVec2};
use crate::positioned::PositioningContext;
use crate::sizing::InlineContentSizesResult;
use crate::{ContainingBlock, DefiniteContainingBlock, IndefiniteContainingBlock};

impl GridFormattingContext {
    /// <https://drafts.csswg.org/css-grid/#layout-algorithm>
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "GridFormattingContext::layout",
            skip_all,
            fields(servo_profiling = true)
        )
    )]
    pub fn layout(
        &self,
        layout_context: &LayoutContext,
        positioning_context: &mut PositioningContext,
        containing_block: &ContainingBlock,
        containing_block_for_container: &ContainingBlock,
    ) -> IndependentLayout {
        let grid_auto_flow = self.style.clone_grid_auto_flow();
        let dimension = match grid_auto_flow {
            GridAutoFlow::ROW => GridDimension::Row,
            // FIXME: dense layout
            GridAutoFlow::COLUMN | GridAutoFlow::DENSE => GridDimension::Column,
            _ => unreachable!(),
        };

        // Step 1. Run the Grid Item Placement Algorithm to resolve the placement of all grid items
        // (including subgrids and their sub-items) in the grid.
        let mut state = GridPlacementContext::new(self);
        state.place_grid_items(&self.children, dimension);
        let placed_grid_items = state.finish();

        // TODO Step 2. Find the size of the grid container, per § 5.2 Sizing Grid Containers.

        // Then, we compute the actual size of those cells and layout the items
        fn tracks_from_definition(
            definition: GenericGridTemplateComponent<LengthPercentage, i32>,
        ) -> Vec<GridTrack> {
            let GenericGridTemplateComponent::TrackList(track_list) = definition else {
                return Vec::new();
            };

            track_list
                .values
                .iter()
                .map(GridTrack::from_definition)
                .collect()
        }

        let row_tracks = tracks_from_definition(self.style.clone_grid_template_rows());
        let column_tracks = tracks_from_definition(self.style.clone_grid_template_columns());

        // Layout rows
        let resolve_only_definite_size = |size| -> Option<Au> {
            if let GenericSize::LengthPercentage(lp) = size {
                Some(Au(0)) // FIXME
            } else {
                None
            }
        };

        let mut layout_context = GridLayoutContext {
            row_tracks,
            column_tracks,
            style: self.style.clone(),
            containing_block,
            min_height: resolve_only_definite_size(self.style.clone_min_height()),
            min_width: resolve_only_definite_size(self.style.clone_min_width()),
            grid_items: placed_grid_items,
        };

        layout_context.run_track_sizing_algorithm(GridDimension::Column);
        layout_context.run_track_sizing_algorithm(GridDimension::Row);

        layout_context.finish()
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "GridFormattingContext::inline_content_sizes",
            skip_all,
            fields(servo_profiling = true)
        )
    )]
    pub fn inline_content_sizes(
        &mut self,
        layout_context: &LayoutContext,
        containing_block_for_children: &IndefiniteContainingBlock,
    ) -> InlineContentSizesResult {
        // https://drafts.csswg.org/css-grid/#intrinsic-sizes
        todo!()
    }
}

/// Temporary state used during grid layout
///
/// Used to place grid items on the [OccupationGrid]
struct GridPlacementContext<'fc> {
    formatting_context: &'fc GridFormattingContext,
    occupation_grid: OccupationGrid,

    /// <https://drafts.csswg.org/css-grid/#auto-placement-cursor>
    auto_placement_cursor: GridCell,

    /// All elements that have already been positioned in the [occupation_grid](Self::occupation_grid)
    placed_grid_items: Vec<GridItemWithGridPlacement>,
}

impl<'fc> GridPlacementContext<'fc> {
    fn new(formatting_context: &'fc GridFormattingContext) -> Self {
        Self {
            formatting_context,
            occupation_grid: OccupationGrid::new(
                formatting_context.explicit_row_count,
                formatting_context.explicit_column_count,
            ),
            placed_grid_items: Vec::default(),
            auto_placement_cursor: GridCell { row: 0, column: 0 },
        }
    }

    /// <https://drafts.csswg.org/css-grid/#auto-placement-algo>
    fn place_grid_items(&mut self, items: &[ArcRefCell<GridItemBox>], dimension: GridDimension) {
        // Step 0. Generate anonymous grid items as described in § 6 Grid Items.
        // NOTE we do that in construct.rs

        // FIXME: Actually implement the rest of this algorithm.
        // We currently place every grid item as if it was automatically positioned
        // (no )
        for item in items {
            self.place_element_with_automatic_grid_position_in_both_axes(item.clone(), dimension);
        }
    }

    fn place_element(&mut self, item: ArcRefCell<GridItemBox>, area: GridArea) {
        log::debug!(target: "grid-layout", "placing grid item at {area:?}");
        self.occupation_grid.mark_area_as_occupied(area);
        let grid_item_with_placement = GridItemWithGridPlacement {
            item,
            placement: area,
        };
        self.placed_grid_items.push(grid_item_with_placement);
    }

    /// Increments the [auto placement cursor](Self::auto_placement_cursor) until it points to a non-occupied
    /// position in the grid that the area can be placedin.
    ///
    /// If this method returns [FoundValidPlacement::Yes] then the [auto placement cursor](Self::auto_placement_cursor)
    /// points to a valid grid position afterwards;
    fn increment_auto_placement_cursor_to_find_position(
        &mut self,
        row_span: i32,
        column_span: i32,
        dimension: GridDimension,
    ) -> FoundValidPlacement {
        if dimension == GridDimension::Row {
            while self.auto_placement_cursor.column + column_span <= self.occupation_grid.max_column
            {
                while self.auto_placement_cursor.row + row_span <= self.occupation_grid.max_row {
                    // Check if the entire item can fit here
                    // TODO this can made more efficient, since we only shift the position by one
                    // per iteration.
                    let mut already_occupied = false;
                    for i in 0..row_span {
                        already_occupied |= self.occupation_grid.is_cell_occupied(GridCell {
                            row: self.auto_placement_cursor.row + i,
                            column: self.auto_placement_cursor.column,
                        });
                    }

                    if !already_occupied {
                        return FoundValidPlacement::Yes;
                    }

                    self.auto_placement_cursor.row += 1;
                }
                self.auto_placement_cursor.column += 1;
                self.auto_placement_cursor.row = self.occupation_grid.min_row;
            }
        } else {
            // TODO
            log::warn!("column major grid layout not implemented");
        }

        FoundValidPlacement::No
    }

    fn place_element_with_automatic_grid_position_in_both_axes(
        &mut self,
        item: ArcRefCell<GridItemBox>,
        dimension: GridDimension,
    ) {
        let row_span = 1;
        let column_span = 1;
        let found_position =
            self.increment_auto_placement_cursor_to_find_position(row_span, column_span, dimension);

        let area = GridArea {
            row_start: self.auto_placement_cursor.row,
            column_start: self.auto_placement_cursor.column,
            row_end: self.auto_placement_cursor.row + row_span,
            column_end: self.auto_placement_cursor.column + column_span,
        };

        self.place_element(item, area);
    }

    fn finish(self) -> Vec<GridItemWithGridPlacement> {
        self.placed_grid_items
    }
}

#[derive(Debug)]
enum FoundValidPlacement {
    Yes,
    No,
}

/// <https://drafts.csswg.org/css-grid/#grid-track>
///
/// Represents either a single column or a single row within the grid
struct GridTrack {
    /// <https://drafts.csswg.org/css-grid/#base-size>
    base_size: Au,

    /// <https://drafts.csswg.org/css-grid/#growth-limit>
    ///
    /// The growth limit can be infinity, in which case we store `None`.
    growth_limit: Option<Au>,

    /// <https://drafts.csswg.org/css-grid/#min-track-sizing-function>
    min_sizing_function: GridTrackSizing,

    /// <https://drafts.csswg.org/css-grid/#max-track-sizing-function>
    max_sizing_function: GridTrackSizing,
}

impl GridTrack {
    fn from_definition(definition: &GenericTrackListValue<LengthPercentage, i32>) -> Self {
        let (min_sizing_function, max_sizing_function) = match definition {
            GenericTrackListValue::TrackSize(track_size) => match track_size {
                GenericTrackSize::FitContent(fit_content) => todo!(),
                GenericTrackSize::Breadth(breadth) => {
                    (breadth.clone().into(), breadth.clone().into())
                },
                GenericTrackSize::Minmax(min, max) => (min.clone().into(), max.clone().into()),
            },
            GenericTrackListValue::TrackRepeat(track_repeat) => {
                todo!()
            },
        };

        Self {
            base_size: Au(0),
            growth_limit: None,
            min_sizing_function,
            max_sizing_function,
        }
    }
}

/// <https://drafts.csswg.org/css-grid/#grid-template-rows-track-sizing-function>
pub enum GridTrackSizing {
    LengthPercentage(LengthPercentage),
    FlexibleLength(f32),
    FitContent,
    MaxContent,
    MinContent,
    Auto,
}

impl GridTrack {
    /// Return how much the track actually grew
    fn attempt_to_grow_by(&mut self, space: Au) -> Au {
        let Some(growth_limit) = self.growth_limit else {
            // If there's no limit then we can grow as much as we like
            self.base_size += space;
            return space;
        };
        let available_growth = growth_limit - self.base_size;
        let grow_by = space.min(available_growth);
        self.base_size += grow_by;

        grow_by
    }
}

impl From<GenericTrackBreadth<LengthPercentage>> for GridTrackSizing {
    fn from(value: GenericTrackBreadth<LengthPercentage>) -> Self {
        match value {
            GenericTrackBreadth::Fr(fr) => Self::FlexibleLength(fr),
            GenericTrackBreadth::Auto => Self::Auto,
            GenericTrackBreadth::MaxContent => Self::MaxContent,
            GenericTrackBreadth::MinContent => Self::MinContent,
            GenericTrackBreadth::Breadth(lp) => Self::LengthPercentage(lp),
        }
    }
}

/// <https://drafts.csswg.org/css-grid/#algo-find-fr-size>
fn find_the_size_of_an_fr(tracks: &[GridTrack], space_to_fill: Au) -> Au {
    // Step 1. Let leftover space be the space to fill minus the base sizes of the non-flexible grid tracks.
    let leftover_space = space_to_fill -
        tracks
            .iter()
            .filter(|t| t.max_sizing_function.flex_factor().is_none())
            .map(|t| t.base_size)
            .sum();

    // Step 2. Let flex factor sum be the sum of the flex factors of the flexible tracks.
    // If this value is less than 1, set it to 1 instead.
    let flex_factor_sum = tracks
        .iter()
        .flat_map(|t| t.max_sizing_function.flex_factor())
        .sum::<f32>()
        .max(1.);

    // Step 3. Let the hypothetical fr size be the leftover space divided by the flex factor sum.
    let hypothetical_fr_size = leftover_space.0 as f32 / flex_factor_sum;

    // TODO Step 4. If the product of the hypothetical fr size and a flexible track’s flex factor is less
    // than the track’s base size, restart this algorithm treating all such tracks as inflexible

    // Step 5. Return the hypothetical fr size.
    Au(hypothetical_fr_size.floor() as i32)
}

impl GridTrackSizing {
    fn flex_factor(&self) -> Option<f32> {
        let Self::FlexibleLength(factor) = self else {
            return None;
        };
        Some(*factor)
    }
}

struct GridLayoutContext<'a> {
    row_tracks: Vec<GridTrack>,
    column_tracks: Vec<GridTrack>,
    style: Arc<ComputedValues>,
    containing_block: &'a ContainingBlock<'a>,
    min_height: Option<Au>,
    min_width: Option<Au>,
    grid_items: Vec<GridItemWithGridPlacement>,
}

impl<'a> GridLayoutContext<'a> {
    fn tracks(&self, dimension: GridDimension) -> &[GridTrack] {
        match dimension {
            GridDimension::Row => &self.row_tracks,
            GridDimension::Column => &self.column_tracks,
        }
    }

    fn tracks_mut(&mut self, dimension: GridDimension) -> &mut [GridTrack] {
        match dimension {
            GridDimension::Row => &mut self.row_tracks,
            GridDimension::Column => &mut self.column_tracks,
        }
    }

    fn content_distribution_property(&self, dimension: GridDimension) -> ContentDistribution {
        match dimension {
            GridDimension::Row => self.style.clone_justify_content().0,
            GridDimension::Column => self.style.clone_align_content().0,
        }
    }

    fn determine_content_height(&self) -> Au {
        let mut total_height = Au(0);
        for track in &self.row_tracks {
            total_height += track.base_size;
        }
        total_height
    }

    /// <https://drafts.csswg.org/css-grid/#algo-track-sizing>
    fn run_track_sizing_algorithm(&mut self, dimension: GridDimension) {
        // Step 1. Initialize Track Sizes
        self.initialize_each_tracks_base_size_and_growth_limit(dimension);

        // FIXME Step 2. Distribute extra space across spanned tracks
        // (https://drafts.csswg.org/css-grid/#extra-space)

        // Step 3. Maximize Tracks
        self.maximize_tracks(dimension);

        // Step 4. Expand Flexible Tracks
        self.expand_flexible_tracks(dimension);

        // Step 5. Expand Stretched auto Tracks
        self.expand_stretched_auto_tracks(dimension);
    }

    /// <https://drafts.csswg.org/css-grid/#algo-init>
    fn initialize_each_tracks_base_size_and_growth_limit(&mut self, dimension: GridDimension) {
        let available_space = self.available_space(dimension).unwrap_or_default();
        let tracks = self.tracks_mut(dimension);

        // Compute base size
        for track in tracks.iter_mut() {
            if let GridTrackSizing::LengthPercentage(length_percentage) = &track.min_sizing_function
            {
                track.base_size = length_percentage.to_used_value(available_space);
            } else {
                track.base_size = Au(0);
            }
        }

        // Compute growth limit
        for track in tracks.iter_mut() {
            if let GridTrackSizing::LengthPercentage(length_percentage) = &track.max_sizing_function
            {
                track.growth_limit = Some(length_percentage.to_used_value(available_space));
            } else {
                track.growth_limit = None;
            }
        }
    }

    /// <https://drafts.csswg.org/css-grid/#algo-grow-tracks>
    fn maximize_tracks(&mut self, dimension: GridDimension) {
        let Some(mut free_space) = self.free_space(dimension).filter(|&s| s > Au(0)) else {
            return;
        };
        let tracks = self.tracks_mut(dimension);

        while free_space.0 as usize > tracks.len() {
            let mut made_progress = false;

            // Amount of au that we are going to give each track
            let au_per_track = free_space / tracks.len() as i32;

            for track in tracks.iter_mut() {
                let grew_by = track.attempt_to_grow_by(au_per_track);
                free_space -= grew_by;

                if grew_by != Au(0) {
                    made_progress = true;
                }
            }

            if !made_progress {
                // All tracks are frozen
                break;
            }
        }
    }

    /// <https://drafts.csswg.org/css-grid/#algo-flex-tracks>
    fn expand_flexible_tracks(&mut self, dimension: GridDimension) {
        let free_space = self.free_space(dimension);

        // If the free space is zero or if sizing the grid container under a min-content constraint:
        let used_flex_fraction = if free_space == Some(Au(0)) {
            // The used flex fraction is zero.
            // NOTE: if the flex fraction is zero then flexible tracks cannot expand
            return;
        }
        // Otherwise, if the free space is a definite length:
        else if free_space.is_some() {
            // The used flex fraction is the result of finding the size of an fr using all of the
            // grid tracks and a space to fill of the available grid space.
            find_the_size_of_an_fr(
                self.tracks(dimension),
                self.available_space(dimension)
                    .expect("Cannot be indefinite"),
            )
        }
        // Otherwise, if the free space is an indefinite length:
        else {
            // TODO flex fraction in indefinitely sized container
            return;
        };

        // For each flexible track, if the product of the used flex fraction and the
        // track’s flex factor is greater than the track’s base size, set its base size to that product.
        for track in self.tracks_mut(dimension).iter_mut() {
            if let Some(flex_factor) = track.max_sizing_function.flex_factor() {
                let flex_size = Au((used_flex_fraction.0 as f32 * flex_factor).floor() as i32);

                if flex_size > track.base_size {
                    track.base_size = flex_size;
                }
            }
        }
    }

    /// <https://drafts.csswg.org/css-grid/#algo-stretch>
    fn expand_stretched_auto_tracks(&mut self, dimension: GridDimension) {
        if !matches!(
            self.content_distribution_property(dimension).primary(),
            AlignFlags::NORMAL | AlignFlags::STRETCH
        ) {
            return;
        }

        let Some(remaining_space) = self.free_space(dimension).or_else(|| {
            // Calculate free space based on min width/height property instead
            let min_size = self.min_size(dimension)?;
            let current_total_size: Au = self
                .tracks(dimension)
                .iter()
                .map(|track| track.base_size)
                .sum();
            Some(min_size - current_total_size)
        }) else {
            return;
        };

        // NOTE: Curiously, the spec does not seem to concern itself with the track growth limit here.
        let tracks = self.tracks_mut(dimension);
        let num_auto_sized_tracks = tracks
            .iter()
            .filter(|track| matches!(track.max_sizing_function, GridTrackSizing::Auto))
            .count();
        if num_auto_sized_tracks == 0 {
            return;
        }

        let au_per_track = remaining_space / num_auto_sized_tracks as i32;

        for track in tracks.iter_mut() {
            if matches!(track.max_sizing_function, GridTrackSizing::Auto) {
                track.base_size += au_per_track;
            }
        }
    }

    /// <https://drafts.csswg.org/css-grid/#available-grid-space>
    fn available_space(&self, dimension: GridDimension) -> Option<Au> {
        match dimension {
            GridDimension::Row => self.containing_block.block_size.non_auto(),
            GridDimension::Column => Some(self.containing_block.inline_size),
        }
    }

    /// Returns either `min-width` or `min-height`
    fn min_size(&self, dimension: GridDimension) -> Option<Au> {
        match dimension {
            GridDimension::Row => self.min_height,
            GridDimension::Column => self.min_width,
        }
    }

    /// <https://drafts.csswg.org/css-grid/#free-space>
    fn free_space(&self, dimension: GridDimension) -> Option<Au> {
        let current_total_size: Au = self
            .tracks(dimension)
            .iter()
            .map(|track| track.base_size)
            .sum();
        let free_space = self.available_space(dimension)? - current_total_size;

        Some(free_space)
    }

    fn for_each_track_spanned_by_placement<F>(
        &self,
        placement: GridArea,
        dimension: GridDimension,
        mut f: F,
    ) where
        F: FnMut(&GridTrack),
    {
        let tracks = self.tracks(dimension);
        let span = placement.span(dimension);
        let start = placement.start(dimension);

        for offset in 0..span {
            let track = &tracks[(start + offset) as usize];
            f(track);
        }
    }

    fn containing_block_size_for_item(&self, placement: GridArea, dimension: GridDimension) -> Au {
        let mut total_space = Au(0);
        self.for_each_track_spanned_by_placement(placement, dimension, |track| {
            total_space += track.base_size
        });

        total_space
    }

    fn compute_absolute_coordinates_for_grid_area(&self, placement: GridArea) -> LogicalRect<Au> {
        // > The contents of a grid container are laid out into a grid, with grid lines
        // > forming the boundaries of each grid items’ containing block.
        let size = LogicalVec2 {
            inline: self.containing_block_size_for_item(placement, GridDimension::Column),
            block: self.containing_block_size_for_item(placement, GridDimension::Row),
        };

        let start_corner = LogicalVec2 {
            inline: self.column_tracks[..placement.column_start as usize]
                .iter()
                .map(|track| track.base_size)
                .sum(),
            block: self.row_tracks[..placement.row_start as usize]
                .iter()
                .map(|track| track.base_size)
                .sum(),
        };

        LogicalRect { start_corner, size }
    }

    fn finish(self) -> IndependentLayout {
        let mut fragments = vec![];
        let content_height = self.determine_content_height();
        let container_writing_mode = self.style.writing_mode;

        for item in &self.grid_items {
            let area = self.compute_absolute_coordinates_for_grid_area(item.placement);
            log::debug!(target: "grid-layout", "Attempting to layout grid item into {:?}", area);

            let containing_block = ContainingBlock {
                inline_size: area.size.inline,
                block_size: area.size.block.into(),
                style: item.item.borrow().style(),
            }
            .into();

            let item_fragments = match item.item.borrow().independent_formatting_context {
                IndependentFormattingContext::NonReplaced(non_replaced) => {
                    let grid_item_layout = non_replaced.layout(
                        layout_context,
                        positioning_context,
                        containing_block_for_children,
                        &containing_block,
                    );
                    grid_item_layout.fragments
                },
                IndependentFormattingContext::Replaced(replaced) => {
                    let size = replaced
                        .contents
                        .used_size_as_if_inline_element_from_content_box_sizes(
                            &containing_block,
                            &replaced.style,
                            LogicalVec2 {
                                inline: AuOrAuto::LengthPercentage(inline_size),
                                block: block_size,
                            },
                            self.content_min_size,
                            self.content_max_size,
                        );

                    replaced.contents.make_fragments(
                        &replaced.style,
                        &containing_block,
                        size.to_physical_size(container_writing_mode),
                    )
                },
            };

            fragments.extend(item_fragments);
        }

        IndependentLayout {
            fragments,
            content_block_size: content_height,
            content_inline_size_for_table: None,
            baselines: Baselines {
                first: None,
                last: None,
            },
        }
    }
}

struct GridItemWithGridPlacement {
    pub item: ArcRefCell<GridItemBox>,
    pub placement: GridArea,
}

impl GridArea {
    fn span(&self, dimension: GridDimension) -> i32 {
        match dimension {
            GridDimension::Row => self.row_end - self.row_start,
            GridDimension::Column => self.column_end - self.column_start,
        }
    }

    fn start(&self, dimension: GridDimension) -> i32 {
        match dimension {
            GridDimension::Row => self.row_start,
            GridDimension::Column => self.column_start,
        }
    }
}
