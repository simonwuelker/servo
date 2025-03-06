/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! <https://svgwg.org/svg2-draft/>

mod construct;

use servo_arc::Arc as ServoArc;
use style::properties::ComputedValues;

use crate::context::LayoutContext;
use crate::formatting_contexts::IndependentLayout;
use crate::layout_box_base::LayoutBoxBase;
use crate::positioned::PositioningContext;
use crate::sizing::InlineContentSizesResult;
use crate::style_ext::LayoutStyle;
use crate::{ConstraintSpace, ContainingBlock};

#[derive(Debug)]
pub(crate) struct SVGFormattingContext {
    style: ServoArc<ComputedValues>,
}

impl SVGFormattingContext {
    pub(crate) fn layout(
        &self,
        layout_context: &LayoutContext,
        positioning_context: &mut PositioningContext,
        containing_block_for_children: &ContainingBlock,
        containing_block: &ContainingBlock,
    ) -> IndependentLayout {
        todo!();
    }

    pub(crate) fn layout_style<'a>(&'a self) -> LayoutStyle<'a> {
        LayoutStyle::Default(&self.style)
    }

    pub(crate) fn compute_inline_content_sizes(
        &self,
        layout_context: &LayoutContext,
        constraint_space: &ConstraintSpace,
    ) -> InlineContentSizesResult {
        todo!()
    }
}
