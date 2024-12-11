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

static ALPHA_LOWERCASE_CHARS: [char; 26] = [
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z',
];

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

use std::iter::{self, FusedIterator};
use std::rc::Rc;

/// <https://drafts.csswg.org/css-lists/#css-counters-set>
#[derive(Debug)]
pub struct CounterSet<'a, Node> {
    /// Counters defined on elements higher up in the DOM tree
    parent: Option<&'a mut Self>,

    /// Counters defined on the preceding sibling of the element (if any)
    sibling: Option<&'a mut [Counter<Node>]>,

    /// Counters defined on the element, but not on its parent
    counters: Vec<Counter<Node>>,
}

#[derive(Clone, Debug, PartialEq)]
enum CounterName {
    ListItem,
    Identifier(Rc<str>),
}

#[derive(Clone, Debug)]
pub struct Counter<Node> {
    /// <https://drafts.csswg.org/css-lists/#css-counter-name>
    name: CounterName,

    /// <https://drafts.csswg.org/css-lists/#css-counter-value>
    value: i32,

    /// <https://drafts.csswg.org/css-lists/#css-counter-creator>
    originating_element: Node,
}

#[derive(Debug)]
pub struct CounterCascadeState<'parent, Node> {
    /// The counters on the parent of the current element
    pub parent: &'parent CounterSet<Node>,

    /// The counters on the preceding sibling of element
    pub sibling: Option<CounterSet<Node>>,
}

impl<'dom, 'parent, Node> CounterSet<'parent, Node>
where
    Node: NodeExt<'dom>,
{
    pub fn new(parent: &'parent CounterSet<Node>) -> Self {
        Self {
            parent,
            sibling: None,
            counters: Vec::default
        }
    }

    pub fn compute_child_counters(&mut self) -> &mut Self {
        self.sibling.insert(self.inherit_counters())
    }

    /// <https://drafts.csswg.org/css-lists/#inherit-counters>
    pub fn inherit_counters(&self) -> CounterSet<Node> {
        // Step 1. If element is the root of its document tree, the element has an initially-empty CSS counters set.
        // Return.
        // NOTE: This is handled at the beginning of the cascade

        // Step 2. Let element counters, representing element’s own CSS counters set, be a copy of the CSS counters
        // set of element’s parent element.
        let mut element_counters = self.parent.clone();

        // Step 3. Let sibling counters be the CSS counters set of element’s preceding sibling (if it has one),
        // or an empty CSS counters set otherwise.
        // For each counter of sibling counters, if element counters does not already contain a counter with the same
        // name, append a copy of counter to element counters.
        if let Some(sibling) = &self.sibling {
            for sibling_counter in &sibling.counters {
                if !element_counters
                    .counters
                    .iter()
                    .find(|c| c.name == sibling_counter.name)
                    .is_some()
                {
                    element_counters.counters.push(sibling_counter.clone());
                }
            }
        }

        // TODO: Step 4. Let value source be the CSS counters set of the element immediately preceding element in tree order.
        // For each source counter of value source, if element counters contains a counter with the same name and
        // creator, then set the value of that counter to source counter’s value.

        element_counters
    }
}

impl<'dom, 'parent, Node> CounterSet<'parent, Node>
where
    Node: NodeExt<'dom>,
{
    /// <https://drafts.csswg.org/css-lists/#instantiate-counter>
    fn instantiate_a_counter(&mut self, name: CounterName, value: i32, originating_element: Node) {
        // Step 2. Let innermost counter be the last counter in counters with the name name.
        // If innermost counter’s originating element is element or a previous sibling of element,
        // remove innermost counter from counters.
        if let Some((index, counter)) = self
            .counters
            .iter()
            .enumerate()
            .find(|(_, c)| c.name == name)
        {
            let is_previous_sibling = counter.originating_element.parent_element() ==
                originating_element.parent_element();
            if counter.originating_element == originating_element || is_previous_sibling {
                println!("Removing counter that is already present on element");
                self.counters.remove(index);
            }
        }

        // Step 3. Append a new counter to counters with name name, originating element element,
        // reversed being reversed, and initial value value (if given)
        let counter = Counter {
            name,
            value,
            originating_element,
        };
        self.counters.push(counter);
    }

    pub(crate) fn resolve_counter(
        &mut self,
        name: &str,
        style: ListStyleType,
        element: Node,
    ) -> String {
        let counter_value = self
            .get(name)
            .map(|counter| counter.value)
            .unwrap_or_else(|| {
                self.instantiate_a_counter(name.into(), 0, element);
                0
            });

        generate_a_counter_representation(counter_value, style)
    }

    /// Update the counter state for an element, given the elements style
    pub fn update(&mut self, info: &NodeAndStyleInfo<Node>) {
        // New counters are instantiated (counter-reset).
        for new_counter in &*info.style.clone_counter_reset() {
            self.instantiate_a_counter(
                new_counter.name.0.as_ref().into(),
                new_counter.value,
                info.node.unwrap(),
            );
        }

        // Counter values are incremented (counter-increment).
        for counter_increment in &*info.style.clone_counter_increment() {
            let name = counter_increment.name.0.as_ref();
            println!(
                "Incrementing counter {name:?}, there are {:?} counters on the element",
                self.counters.len()
            );
            if let Some(counter) = self.get(name) {
                counter.value += 1;
                println!("incremented existing counter");
            } else {
                println!("instantiated new counter");
                self.instantiate_a_counter(name.into(), 1, info.node.unwrap());
            }
        }

        // Counter values are explicitly set (counter-set).
        for counter_set in &*info.style.clone_counter_set() {
            let name = counter_set.name.0.as_ref();
            if let Some(counter) = self.get(name) {
                counter.value = counter_set.value;
            } else {
                self.instantiate_a_counter(name.into(), counter_set.value, info.node.unwrap());
            }
        }

        // List items additionally increment the implicit list-item counter
        // https://drafts.csswg.org/css-lists/#list-item-counter
        if info.style.get_box().display.is_list_item() {
            // list-item counter is incremented by 1, unless the counter-increment property
            // says otherwise
            let increment_by = info
                .style
                .clone_counter_increment()
                .iter()
                .find(|counter_increment| counter_increment.name.0.as_ref() == "list-item")
                .map(|increment| increment.value)
                .unwrap_or(1);

            if let Some(counter) = self
                .counters
                .iter_mut()
                .find(|counter| matches!(counter.name, CounterName::ListItem))
            {
                counter.value += increment_by;
            } else {
                self.instantiate_a_counter(CounterName::ListItem, increment_by, info.node.unwrap());
            }
        }
    }
}

impl<'a, Node> Default for CounterSet<'a, Node> {
    fn default() -> Self {
        Self {
            counters: Vec::default(),
            sibling: None,
            parent: None,
        }
    }
}

/// <https://drafts.csswg.org/css-counter-styles-3/#generate-a-counter>
fn generate_a_counter_representation(value: i32, style: ListStyleType) -> String {
    let mut style =
        SupportedCounterStyle::try_from(style).unwrap_or(SupportedCounterStyle::Decimal);

    let mut representation = loop {
        if let Ok(representation) = style.generate_representation(value.abs()) {
            break representation;
        } else {
            // TODO: Use fallback style here when supported
            style = SupportedCounterStyle::Decimal;
        }
    };

    // TODO: Don't add no break space here
    representation.push('\u{00a0}');

    // Step 6. Return the representation.
    return representation;
}

enum SupportedCounterStyle {
    Symbol(char),
    LowerAlpha,
    UpperAlpha,
    Decimal,
}

struct ValueOutOfRange;

impl SupportedCounterStyle {
    fn generate_representation(&self, value: i32) -> Result<String, ValueOutOfRange> {
        // TODO: Handle negative numbers correctly
        let value = value.abs() as usize;

        let representation = match self {
            Self::Symbol(c) => generate_symbolic_counter(value, &[*c]),
            Self::Decimal => generate_numeric_counter(value),
            Self::LowerAlpha => generate_alphabetic_counter(value, &ALPHA_LOWERCASE_CHARS),
            Self::UpperAlpha => todo!(),
        };

        Ok(representation)
    }
}

/// Indicates that a particular list style is not yet supported
struct NotSupported;

impl TryFrom<ListStyleType> for SupportedCounterStyle {
    type Error = NotSupported;

    fn try_from(value: ListStyleType) -> Result<Self, Self::Error> {
        let style = match value {
            ListStyleType::Disc => Self::Symbol('•'),
            ListStyleType::Circle => Self::Symbol('◦'),
            ListStyleType::Square => Self::Symbol('▪'),
            ListStyleType::DisclosureOpen => Self::Symbol('▾'),
            ListStyleType::DisclosureClosed => Self::Symbol('‣'),
            ListStyleType::Decimal => Self::Decimal,
            ListStyleType::LowerAlpha => Self::LowerAlpha,
            ListStyleType::UpperAlpha => Self::UpperAlpha,
            _ => return Err(NotSupported),
        };

        Ok(style)
    }
}

/// <https://drafts.csswg.org/css-counter-styles-3/#cyclic-system>
fn generate_cyclic_counter(value: usize, symbols: &[char]) -> String {
    let index = (value - 1) % symbols.len();
    symbols[index].into()
}

// /// <https://drafts.csswg.org/css-counter-styles-3/#numeric-system>
fn generate_numeric_counter(value: usize) -> String {
    // FIXME
    format!("{value}")
}

/// <https://drafts.csswg.org/css-counter-styles-3/#symbolic-system>
fn generate_symbolic_counter(value: usize, symbols: &[char]) -> String {
    let symbol = symbols[(value - 1) % symbols.len()];
    let repetitions = (value + symbols.len() - 1) / symbols.len();

    iter::repeat(symbol).take(repetitions).collect()
}

/// <https://drafts.csswg.org/css-counter-styles-3/#valdef-counter-style-system-alphabetic>
fn generate_alphabetic_counter(value: usize, symbols: &[char]) -> String {
    let n = symbols.len() as usize;
    let mut result = String::new();
    let mut remaining = value + 1;
    while remaining != 0 {
        remaining -= 1;
        result.insert(0, symbols[(remaining % n) as usize]);
        remaining /= n;
    }
    result
}

impl<'a> PartialEq<&'a str> for CounterName {
    fn eq(&self, other: &&'a str) -> bool {
        match self {
            Self::ListItem => false,
            Self::Identifier(ident) => ident.as_ref() == *other,
        }
    }
}

impl<'a> From<&'a str> for CounterName {
    fn from(value: &'a str) -> Self {
        Self::Identifier(Rc::from(value))
    }
}

enum CounterSetIteratorState {
    ElementCounters(usize),
    SiblingCounters(usize),
}

struct CounterSetIterator<'set, 'parent, Node> {
    counter: &'set mut CounterSet<'parent, Node>,
    state: CounterSetIteratorState,
}

impl<'parent, Node> CounterSet<'parent, Node> {
    fn iter_mut<'set>(&'set mut self) -> CounterSetIterator<'set, 'parent, Node> {
        CounterSetIterator {
            counter: self,
            state: CounterSetIteratorState::ElementCounters(0),
        }
    }

    fn find(&mut self, name: CounterName) -> Option<&'_ mut Counter<Node>> {
        self.counters
            .iter_mut()
            .find(|counter| counter.name == name)
            .or_else(|| {
                self.sibling?
                    .iter_mut()
                    .find(|counter| counter.name == name)
            })
            .or_else(|| self.parent?.find(name))
    }
}

// impl<'set, 'parent, Node> Iterator for CounterSetIterator<'set, 'parent, Node> {
//     type Item = &'parent mut Counter<Node>;

//     fn next(&mut self) -> Option<Self::Item> {
//         match &mut self.state {
//             CounterSetIteratorState::ElementCounters(index) => {
//                 let Some(element) = self.counter.counters.get_mut(*index) else {
//                     // No more counters on the element, move on to its sibling
//                     self.state = CounterSetIteratorState::SiblingCounters(0);
//                     return self.next();
//                 };
//                 *index += 1;

//                 Some(element)
//             },
//             CounterSetIteratorState::SiblingCounters(index) => {
//                 let Some(sibling_counters) = &mut self.counter.sibling else {
//                     // No siblings, move on to the parent
//                     *self = self.counter.parent.as_mut()?.iter_mut();
//                     return self.next();
//                 };

//                 let Some(element) = sibling_counters.get_mut(*index) else {
//                     // No more counters on the sibling, move on to the parent
//                     *self = self.counter.parent.as_mut()?.iter_mut();
//                     return self.next();
//                 };
//                 *index += 1;

//                 Some(element)
//             }

//         }
//     }
// }

impl<'set, 'parent, Node> FusedIterator for CounterSetIterator<'set, 'parent, Node> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cyclic_counter_generation() {
        let symbols = &['a', 'b', 'c'];

        assert_eq!(generate_cyclic_counter(0, symbols), "a");
        assert_eq!(generate_cyclic_counter(1, symbols), "b");
        assert_eq!(generate_cyclic_counter(2, symbols), "c");
        assert_eq!(generate_cyclic_counter(3, symbols), "a");
    }

    #[test]
    fn symbolic_counter_generation() {
        let symbols = &['a', 'b', 'c'];

        assert_eq!(generate_alphabetic_counter(0, symbols), "a");
        assert_eq!(generate_alphabetic_counter(1, symbols), "b");
        assert_eq!(generate_alphabetic_counter(2, symbols), "c");
        assert_eq!(generate_alphabetic_counter(3, symbols), "aa");
        assert_eq!(generate_alphabetic_counter(3, symbols), "bb");
    }

    #[test]
    fn alphabetic_counter_generation() {
        let symbols = &['a', 'b', 'c'];

        assert_eq!(generate_alphabetic_counter(0, symbols), "a");
        assert_eq!(generate_alphabetic_counter(1, symbols), "b");
        assert_eq!(generate_alphabetic_counter(2, symbols), "c");
        assert_eq!(generate_alphabetic_counter(3, symbols), "aa");
        assert_eq!(generate_alphabetic_counter(3, symbols), "ab");
    }
}
