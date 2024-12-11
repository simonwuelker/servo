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

use std::cell::Cell;
use std::iter::{self};
use std::mem;
use std::rc::Rc;

/// <https://drafts.csswg.org/css-lists/#css-counters-set>
///
/// CounterSets form a stack-allocated linked list of counter definitions
#[derive(Debug)]
pub struct CounterSet<'a, Node> {
    /// Counters defined on elements higher up in the DOM tree
    parent: Option<&'a CounterSet<'a, Node>>,

    /// Counters defined on the preceding sibling of the element (if any)
    sibling: Option<Vec<Counter<Node>>>,

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
    ///
    /// The value is wrapped in a `Cell` so the nodes of the [CounterSet-chain](CounterSet) can
    /// hold immutable references to their predecessors. Mutable references are not possible here,
    /// because they are invariant to their lifetimes.
    value: Cell<i32>,

    /// <https://drafts.csswg.org/css-lists/#css-counter-creator>
    originating_element: Node,
}

impl<'dom, 'parent, Node> CounterSet<'parent, Node>
where
    Node: NodeExt<'dom>,
{
    pub fn new(parent: &'parent Self) -> Self {
        Self {
            parent: Some(parent),
            sibling: None,
            counters: Vec::default()
        }
    }

    pub fn next_sibling<'a>(&'a mut self) -> &'a mut CounterSet<'parent, Node> {
        self.sibling = Some(mem::take(&mut self.counters));

        self
    }

    fn find(&self, name: CounterName) -> Option<&'_ Counter<Node>> {
        self.counters
            .iter()
            .find(|counter| counter.name == name)
            .or_else(|| {
                self.sibling.as_ref()?
                    .iter()
                    .find(|counter| counter.name == name)
            })
            .or_else(|| {
                println!("Searching on parent, which has {:?} counters...", &self.parent.as_ref()?.counters);
                self.parent.as_ref()?.find(name)
            })
    }

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
            value: Cell::new(value),
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
            .find(name.into())
            .map(|counter| counter.value.get())
            .unwrap_or_else(|| {
                self.instantiate_a_counter(name.into(), 0, element);
                0
            });

        generate_a_counter_representation(counter_value, style)
    }

    /// Update the counter state for an element, given the elements style
    pub fn update(&mut self, info: &NodeAndStyleInfo<Node>) {
        println!("=== Updating Counters for element {:?} ===", info.node.unwrap().type_id());
        // New counters are instantiated (counter-reset).
        for new_counter in &*info.style.clone_counter_reset() {
            println!("Instantiated new counter {:?} on {:?}", new_counter.name.0.as_ref(), info.node.unwrap().type_id());
            println!();
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
            if let Some(counter) = self.find(name.into()) {
                counter.value.set(counter.value.get() + 1);
                println!("incremented existing counter");
            } else {
                println!("instantiated new counter");
                self.instantiate_a_counter(name.into(), 1, info.node.unwrap());
            }
            println!();
        }

        // Counter values are explicitly set (counter-set).
        for counter_set in &*info.style.clone_counter_set() {
            let name = counter_set.name.0.as_ref();
            if let Some(counter) = self.find(name.into()) {
                counter.value.set(counter_set.value);
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
                counter.value.set(counter.value.get() + increment_by);
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
