/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::borrow::Cow;

use rayon::iter::{IntoParallelIterator, ParallelIterator};
use style::values::specified::TextDecorationLine;

use super::{GridFormattingContext, GridItemBox};
use crate::cell::ArcRefCell;
use crate::context::LayoutContext;
use crate::dom::{BoxSlot, LayoutBox, NodeExt};
use crate::dom_traversal::{Contents, NodeAndStyleInfo, NonReplacedContents, TraversalHandler};
use crate::flow::inline::construct::InlineFormattingContextBuilder;
use crate::flow::{BlockContainer, BlockFormattingContext};
use crate::formatting_contexts::{
    IndependentFormattingContext, NonReplacedFormattingContext,
    NonReplacedFormattingContextContents,
};
use crate::style_ext::DisplayGeneratingBox;

impl GridFormattingContext {
    pub fn construct<'dom>(
        context: &LayoutContext,
        info: &NodeAndStyleInfo<impl NodeExt<'dom>>,
        contents: NonReplacedContents,
        propagated_text_decoration_line: TextDecorationLine,
    ) -> Self {
        let text_decoration_line =
            propagated_text_decoration_line | info.style.clone_text_decoration_line();
        let mut builder = GridFormattingContextBuilder {
            context,
            info,
            text_decoration_line,
            contiguous_text_runs: Vec::new(),
            jobs: Vec::new(),
            has_text_runs: false,
        };
        contents.traverse(context, info, &mut builder);
        builder.finish()
    }
}

/// <https://drafts.csswg.org/css-grid/#grid-items>
struct GridFormattingContextBuilder<'a, 'dom, Node> {
    context: &'a LayoutContext<'a>,
    info: &'a NodeAndStyleInfo<Node>,
    text_decoration_line: TextDecorationLine,
    contiguous_text_runs: Vec<GridTextRun<'dom, Node>>,
    jobs: Vec<GridLevelJob<'dom, Node>>,
    has_text_runs: bool,
}

struct GridTextRun<'dom, Node> {
    info: NodeAndStyleInfo<Node>,
    text: Cow<'dom, str>,
}

enum GridLevelJob<'dom, Node> {
    /// Or pseudo-element
    Element {
        info: NodeAndStyleInfo<Node>,
        display: DisplayGeneratingBox,
        contents: Contents,
        box_slot: BoxSlot<'dom>,
    },
    TextRuns(Vec<GridTextRun<'dom, Node>>),
}

impl<'a, 'dom, Node: 'dom> TraversalHandler<'dom, Node>
    for GridFormattingContextBuilder<'a, 'dom, Node>
where
    Node: NodeExt<'dom>,
{
    fn handle_text(&mut self, info: &NodeAndStyleInfo<Node>, text: Cow<'dom, str>) {
        self.contiguous_text_runs.push(GridTextRun {
            info: info.clone(),
            text,
        })
    }

    /// Or pseudo-element
    fn handle_element(
        &mut self,
        info: &NodeAndStyleInfo<Node>,
        display: DisplayGeneratingBox,
        contents: Contents,
        box_slot: BoxSlot<'dom>,
    ) {
        // FIXME: are text runs considered "contiguous" if they are only separated
        // by an out-of-flow abspos element?
        // (That is, are they wrapped in the same anonymous flex item, or each its own?)
        self.wrap_any_text_in_anonymous_block_container();

        self.jobs.push(GridLevelJob::Element {
            info: info.clone(),
            display,
            contents,
            box_slot,
        })
    }
}

impl<'a, 'dom, Node> GridFormattingContextBuilder<'a, 'dom, Node>
where
    Node: NodeExt<'dom>,
{
    fn wrap_any_text_in_anonymous_block_container(&mut self) {
        let runs = std::mem::take(&mut self.contiguous_text_runs);
        let contains_non_whitespace = !runs.iter().all(GridTextRun::has_only_document_white_space);
        if contains_non_whitespace {
            self.jobs.push(GridLevelJob::TextRuns(runs));
            self.has_text_runs = true;
        }
    }

    fn finish(mut self) -> GridFormattingContext {
        self.wrap_any_text_in_anonymous_block_container();

        let anonymous_style = if self.has_text_runs {
            Some(
                self.context
                    .shared_context()
                    .stylist
                    .style_for_anonymous::<Node::ConcreteElement>(
                        &self.context.shared_context().guards,
                        &style::selector_parser::PseudoElement::ServoAnonymousBox,
                        &self.info.style,
                    ),
            )
        } else {
            None
        };

        let mut children: Vec<_> = std::mem::take(&mut self.jobs)
            .into_par_iter()
            .filter_map(|job| match job {
                GridLevelJob::TextRuns(runs) => {
                    let mut inline_formatting_context_builder =
                        InlineFormattingContextBuilder::new();
                    for flex_text_run in runs.into_iter() {
                        inline_formatting_context_builder
                            .push_text(flex_text_run.text, &flex_text_run.info);
                    }

                    let inline_formatting_context = inline_formatting_context_builder.finish(
                        self.context,
                        self.text_decoration_line,
                        true,
                        false,
                        self.info.style.writing_mode.to_bidi_level(),
                    )?;

                    let block_formatting_context = BlockFormattingContext::from_block_container(
                        BlockContainer::InlineFormattingContext(inline_formatting_context),
                    );
                    let info = &self.info.new_anonymous(anonymous_style.clone().unwrap());
                    let non_replaced = NonReplacedFormattingContext {
                        base_fragment_info: info.into(),
                        style: info.style.clone(),
                        content_sizes_result: None,
                        contents: NonReplacedFormattingContextContents::Flow(
                            block_formatting_context,
                        ),
                    };

                    let independent_formatting_context =
                        IndependentFormattingContext::NonReplaced(non_replaced);

                    Some(ArcRefCell::new(GridItemBox {
                        independent_formatting_context,
                    }))
                },
                GridLevelJob::Element {
                    info,
                    display,
                    contents,
                    box_slot,
                } => {
                    let display_inside = match display {
                        DisplayGeneratingBox::OutsideInside { inside, .. } => inside,
                        DisplayGeneratingBox::LayoutInternal(_) => display.display_inside(),
                    };
                    let box_ = if info.style.get_box().position.is_absolutely_positioned() {
                        todo!()
                    } else {
                        let independent_formatting_context =
                            IndependentFormattingContext::construct(
                                self.context,
                                &info,
                                display_inside,
                                contents,
                                self.text_decoration_line,
                            );

                        ArcRefCell::new(GridItemBox {
                            independent_formatting_context,
                        })
                    };
                    box_slot.set(LayoutBox::GridLevel(box_.clone()));
                    Some(box_)
                },
            })
            .collect();

        // https://drafts.csswg.org/css-display-4/#order-modified-document-order
        children.sort_by_key(|child| (&*child.borrow()).style().clone_order());

        GridFormattingContext::new(self.info.style.clone(), children)
    }
}

impl<'dom, Node> GridTextRun<'dom, Node> {
    /// <https://drafts.csswg.org/css-text/#white-space>
    fn has_only_document_white_space(&self) -> bool {
        // FIXME: is this the right definition? See
        // https://github.com/w3c/csswg-drafts/issues/5146
        // https://github.com/w3c/csswg-drafts/issues/5147
        self.text
            .bytes()
            .all(|byte| matches!(byte, b' ' | b'\n' | b'\t'))
    }
}
