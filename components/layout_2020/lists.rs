/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use log::warn;
use style::properties::longhands::list_style_type::computed_value::T as ListStyleType;
use style::properties::style_structs;
use style::values::computed::Image;

use crate::context::LayoutContext;
use crate::dom::NodeExt;
use crate::dom_traversal::{NodeAndStyleInfo, PseudoElementContentItem};
use crate::replaced::ReplacedContent;

/// <https://drafts.csswg.org/css-lists/#content-property>
pub(crate) fn make_marker<'dom, Node>(
    context: &LayoutContext,
    info: &NodeAndStyleInfo<Node>,
) -> Option<Vec<PseudoElementContentItem>>
where
    Node: NodeExt<'dom>,
{
    let style = info.style.get_list();
    let node = match info.node {
        Some(node) => node,
        None => {
            warn!("Tried to make a marker for an anonymous node!");
            return None;
        },
    };

    // https://drafts.csswg.org/css-lists/#marker-image
    let marker_image = || match &style.list_style_image {
        Image::Url(url) => Some(vec![
            PseudoElementContentItem::Replaced(ReplacedContent::from_image_url(
                node, context, url,
            )?),
            PseudoElementContentItem::Text(" ".into()),
        ]),
        // XXX: Non-None image types unimplemented.
        Image::ImageSet(..) |
        Image::Gradient(..) |
        Image::CrossFade(..) |
        Image::PaintWorklet(..) |
        Image::None => None,
    };
    marker_image().or_else(|| {
        Some(vec![PseudoElementContentItem::Text(
            marker_string(style)?.into(),
        )])
    })
}

/// <https://drafts.csswg.org/css-lists/#marker-string>
fn marker_string(style: &style_structs::List) -> Option<&'static str> {
    match style.list_style_type {
        ListStyleType::None => None,
        // TODO: Using non-breaking space here is a bit of a hack to give a bit of margin to outside
        // markers, but really we should be setting `white-space: pre` on them instead.
        // See https://github.com/w3c/csswg-drafts/issues/4891.
        ListStyleType::Disc => Some("•\u{00a0}"),
        ListStyleType::Circle => Some("◦\u{00a0}"),
        ListStyleType::Square => Some("▪\u{00a0}"),
        ListStyleType::DisclosureOpen => Some("▾\u{00a0}"),
        ListStyleType::DisclosureClosed => Some("‣\u{00a0}"),
        ListStyleType::Decimal |
        ListStyleType::LowerAlpha |
        ListStyleType::UpperAlpha |
        ListStyleType::ArabicIndic |
        ListStyleType::Bengali |
        ListStyleType::Cambodian |
        ListStyleType::CjkDecimal |
        ListStyleType::Devanagari |
        ListStyleType::Gujarati |
        ListStyleType::Gurmukhi |
        ListStyleType::Kannada |
        ListStyleType::Khmer |
        ListStyleType::Lao |
        ListStyleType::Malayalam |
        ListStyleType::Mongolian |
        ListStyleType::Myanmar |
        ListStyleType::Oriya |
        ListStyleType::Persian |
        ListStyleType::Telugu |
        ListStyleType::Thai |
        ListStyleType::Tibetan |
        ListStyleType::CjkEarthlyBranch |
        ListStyleType::CjkHeavenlyStem |
        ListStyleType::LowerGreek |
        ListStyleType::Hiragana |
        ListStyleType::HiraganaIroha |
        ListStyleType::Katakana |
        ListStyleType::KatakanaIroha => {
            // TODO: Implement support for counters.
            None
        },
    }
}

use std::rc::Rc;

/// <https://drafts.csswg.org/css-lists/#css-counters-set>
#[derive(Clone, Debug, Default)]
pub struct CounterSet {
    counters: Vec<Counter>,
}

#[derive(Clone, Debug)]
pub struct Counter {
    /// The counter name
    ///
    /// Since many elements are going to share the same counter (with potentially different values), this is an `Rc`
    name: Rc<str>,
    value: i32,
}

#[derive(Clone, Debug, Default)]
pub struct CounterCascadeState {
    /// The counters on the parent of the current element
    pub parent_counters: CounterSet,

    /// The counters on the preceding sibling of element, if any
    pub sibling: Option<CounterSet>,

    /// The counters on the element directly preceding the current element,
    /// in tree order
    pub preceding_element: CounterSet,
}

impl CounterCascadeState {
    /// <https://drafts.csswg.org/css-lists/#inherit-counters>
    pub fn inherit_counters(&self) -> CounterSet {
        // Step 1. If element is the root of its document tree, the element has an initially-empty CSS counters set.
        // Return.
        // NOTE: This is handled at the beginning of the cascade

        // Step 2. Let element counters, representing element’s own CSS counters set, be a copy of the CSS counters
        // set of element’s parent element.
        let mut element_counters = self.parent_counters.clone();

        // Step 3. Let sibling counters be the CSS counters set of element’s preceding sibling (if it has one),
        // or an empty CSS counters set otherwise.
        // For each counter of sibling counters, if element counters does not already contain a counter with the same
        // name, append a copy of counter to element counters.
        if let Some(sibling) = &self.sibling {
            for sibling_counter in &sibling.counters {
                if !element_counters.counters.iter().find(|c| c.name == sibling_counter.name).is_some() {
                    element_counters.counters.push(sibling_counter.clone());
                }
            }
        }

        // Step 4. Let value source be the CSS counters set of the element immediately preceding element in tree order.
        // For each source counter of value source, if element counters contains a counter with the same name and
        // creator, then set the value of that counter to source counter’s value.
        element_counters
    }
}

impl CounterSet {
    /// <https://drafts.csswg.org/css-lists/#instantiate-counter>
    pub fn instantiate_a_counter(&mut self, element: Node, counter: Counter) {
    }
}