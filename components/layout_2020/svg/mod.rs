/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! <https://svgwg.org/svg2-draft/>

mod construct;

use app_units::Au;
use euclid::{Rect, Size2D};
use html5ever::{local_name, ns};
use servo_arc::Arc as ServoArc;
use style::dom::{TElement, TNode};
use style::properties::ComputedValues;
use style_traits::CSSPixel;

use crate::context::LayoutContext;
use crate::dom::NodeExt;
use crate::formatting_contexts::IndependentLayout;
use crate::fragment_tree::Fragment;
use crate::geom::PhysicalSize;
use crate::positioned::PositioningContext;
use crate::sizing::InlineContentSizesResult;
use crate::style_ext::LayoutStyle;
use crate::{ConstraintSpace, ContainingBlock};

#[derive(Debug)]
pub(crate) struct SVGFormattingContext {
    /// Stores the elements in paint order
    ///
    /// All SVG elements are assigned a fixed position inside the svg viewport,
    /// so the structure of the DOM tree does not need to be preserved here.
    children: Vec<SVGElement>,
}

#[derive(Debug)]
enum SVGElement {
    Circle { style: ServoArc<ComputedValues> },
}

impl SVGFormattingContext {
    pub(crate) fn build<'dom>(element: impl NodeExt<'dom>, context: &LayoutContext) -> Self {
        let mut svg_children = vec![];
        for child in element.dom_children().flat_map(|node| node.as_element()) {
            if !child.is_svg_element() {
                continue;
            }

            let svg_element = if child.local_name() == "circle" {
                SVGElement::Circle {
                    style: element.style(context),
                }
            } else {
                continue;
            };

            svg_children.push(svg_element);
        }

        Self {
            children: svg_children,
        }
    }

    pub(crate) fn make_fragments(&self, size: Rect<Au, CSSPixel>) -> Vec<Fragment> {
        self.svg_children
            .iter()
            .map(SVGElement::make_fragment)
            .collect()
    }

    pub(crate) fn natural_size_in_dots(&self) -> Option<Size2D<f64, CSSPixel>> {
        // FIXME
        None
    }
}

impl SVGElement {
    fn make_fragment(&self) -> Fragment {
        match self {
            Self::Circle { style } => {
                // Circle elements are translated into an equivalent path
            },
        }
    }
}
