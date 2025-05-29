/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::str::FromStr;
use std::borrow::Cow;

use script_bindings::inheritance::Castable;
use url::Url;
use cssparser::ParserInput;
use script_bindings::codegen::InheritTypes::CharacterDataTypeId;
use script_bindings::codegen::InheritTypes::HTMLElementTypeId;
use script_bindings::codegen::InheritTypes::ElementTypeId;
use script_bindings::codegen::InheritTypes::NodeTypeId;
use script_bindings::codegen::GenericBindings::DocumentBinding::DocumentMethods;
use script_bindings::codegen::GenericBindings::HTMLElementBinding::HTMLElementMethods;
use script_bindings::codegen::GenericBindings::SelectionBinding::SelectionMethods;
use script_bindings::root::DomRoot;
use script_bindings::script_runtime::CanGc;
use script_bindings::str::DOMString;
use style::color::parsing::parse_color_with;
use style::context::QuirksMode;
use style::parser::ParserContext;
use style::stylesheets::{Namespaces, Origin, UrlExtraData};
use style::values::Parser;
use style_traits::ParsingMode;
use style::values::specified::Color;

use crate::dom::text::Text;
use crate::dom::node::Node;
use crate::dom::node::TreeIterator;
use crate::dom::range::Range;
use crate::dom::document::Document;
use crate::dom::htmlelement::HTMLElement;
use crate::dom::node::ShadowIncluding;

/// <https://w3c.github.io/editing/docs/execCommand/#command>
///
/// To query whether or not a [Command] is [supported], call `from_str`.
///
/// [supported]: https://w3c.github.io/editing/docs/execCommand/#supported
#[derive(Clone, Copy, Debug)]
pub(crate) enum Command {
    /// <https://w3c.github.io/editing/docs/execCommand/#the-backcolor-command>
    BackColor,

    HiliteColor,
}

impl Command {
    /// <https://w3c.github.io/editing/docs/execCommand/#enabled>
    pub(crate) fn is_enabled(&self, document: &Document, can_gc: CanGc) -> bool {
        match self {
            Self::BackColor => {
                // Described in https://w3c.github.io/editing/docs/execCommand/#enabled under
                // non-miscellaneous commands.
                let Some(common_editing_host) =
                    Self::get_editing_host_for_selection(document, can_gc)
                else {
                    return false;
                };

                // TODO: return false if the editing host of either the start or end node
                // is an EditContext editing host.

                // This command must not be enabled if the editing host is in the plaintext-only state.
                common_editing_host.ContentEditable().str() != "plaintext-only"
            },
        }
    }

    /// Return `true` iff the command is in the [miscellaneous commands] section.
    ///
    /// [miscellaneous commands]: https://w3c.github.io/editing/docs/execCommand/#miscellaneous-commands
    pub(crate) fn is_miscellaneous(&self) -> bool {
        // We don't support any of these commands
        false
    }

    /// Returns the first editing host among the current selection's start and end node's common ancestors,
    /// if any.
    pub(crate) fn get_editing_host_for_selection(
        document: &Document,
        can_gc: CanGc,
    ) -> Option<DomRoot<HTMLElement>> {
        let active_range = get_active_range(document, can_gc)?;

        let start_node = active_range.start_container();
        let end_node = active_range.end_container();

        if !start_node.is_editing_host_or_editable() || end_node.is_editing_host_or_editable() {
            return None;
        }

        // Find the common editing host, if any
        let Some(common_ancestor) = start_node.common_ancestor(&end_node, ShadowIncluding::No)
        else {
            return None;
        };

        common_ancestor
            .inclusive_ancestors(ShadowIncluding::No)
            .filter_map(|ancestor| DomRoot::downcast::<HTMLElement>(ancestor))
            .find(|html_element| html_element.is_editing_host())
    }

    pub(crate) fn take_action(&self, document: &Document, value: DOMString, can_gc: CanGc) -> bool {
        match self {
            Self::BackColor => {
                let mut used_value = value;

                // Step 1. If value is not a valid CSS color, prepend "#" to it.
                let bogus_url: UrlExtraData = Url::from_str("http://example.com").unwrap().into();
                let parser_context = ParserContext::new(
                    Origin::Author,
                    &bogus_url,
                    None,
                    ParsingMode::DEFAULT,
                    QuirksMode::NoQuirks,
                    Cow::Owned(Namespaces::default()),
                    None,
                    None,
                );
                let mut input = ParserInput::new(&value);
                let mut parser = Parser::new(&mut input);
                if parse_color_with(&parser_context, &mut parser).is_err() {
                    used_value = format!("#{}", value.str()).into();

                    // Step 2. If value is still not a valid CSS color, or if it is currentColor, return false.
                    let mut input = ParserInput::new(&value);
                    let mut parser = Parser::new(&mut input);
                    if matches!(
                        parse_color_with(&parser_context, &mut parser),
                        Ok(Color::CurrentColor) | Err(_)
                    ) {
                        return false;
                    }
                }

                // Step 3. Set the selection's value to value.
                set_the_selections_value(document, *self, &used_value, can_gc);

                // Step 4. Return true.
                true
            },
        }
    }

    /// <https://w3c.github.io/editing/docs/execCommand/#dfn-map-an-edit-command-to-input-type-value>
    pub(crate) fn mapped_value(&self) -> &'static str {
        match self {
            Self::BackColor => "formatBackColor",
        }
    }
}

struct InvalidCommandId;

impl FromStr for Command {
    type Err = InvalidCommandId;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input == "backColor" {
            Ok(Self::BackColor)
        } else {
            Err(InvalidCommandId)
        }
    }
}

/// <https://w3c.github.io/editing/docs/execCommand/#set-the-selection's-value>
fn set_the_selections_value(document: &Document, command: Command, new_value: Option<&str>, can_gc: CanGc) {
    // Step 1. Let command be the current command.

    // Step 2. If there is no formattable node effectively contained in the active range:
    fn no_formattable_node_steps() {
        // TODO: Step 2.1 If command has inline command activated values, set the state override to
        // true if new value is among them and false if it's not.

        // TODO: Step 2.2 If command is "subscript", unset the state override for "superscript".

        // TODO: Step 2.3 If command is "superscript", unset the state override for "subscript".

        // TODO: Step 2.4 If new value is null, unset the value override (if any).

        // TODO: Step 2.5 Otherwise, if command is "createLink" or it has a value specified,
        // set the value override to new value.

        // Step 2.6 Abort these steps.
        // NOTE: Done by the caller
    }
    let Some(active_range) = get_active_range(document, can_gc) else {
        no_formattable_node_steps();
        return;
    };
    let has_formattable_node = effectively_contained_nodes(&active_range)
        .any(|node| is_formattable_node(&node));
    if !has_formattable_node {
        no_formattable_node_steps();
        return;
    }

    // Step 3. If the active range's start node is an editable Text node, and its start offset is
    // neither zero nor its start node's length, call splitText() on the active range's start node,
    // with argument equal to the active range's start offset. Then set the active range's start node
    // to the result, and its start offset to zero.
    let start_container = active_range.start_container();
    if start_container.is_editable() && active_range.start_offset() != 0 && active_range.start_offset() != start_container.len() {
        if let Some(text) = start_container.downcast::<Text>() {
            let start = text.SplitText(active_range.start_offset());
            active_range.set_start(start, 0);
        }
    }

    // Step 4. If the active range's end node is an editable Text node, and its end offset is neither zero
    // nor its end node's length, call splitText() on the active range's end node, with argument equal
    // to the active range's end offset.
    let end_container = active_range.end_container();
    if end_container.is_editable() && active_range.end_offset() != 0 && active_range.end() != end_container.len() {
        if let Some(text) = end_container.downcast::<Text>() {
            text.SplitText(active_range.end_offset());
        }
    }

    // Step 5. Let element list be all editable Elements effectively contained in the active range.
    // Step 6. For each element in element list, clear the value of element.
    for element in effectively_contained_nodes(&active_range).filter_map(|node| DomRoot::downcast::<Element>(node)) {
        clear_the_value_of(&element);
    }

    todo!()
}

/// <https://w3c.github.io/editing/docs/execCommand/#active-range>
fn get_active_range(document: &Document, can_gc: CanGc) -> Option<DomRoot<Range>> {
    document
        .GetSelection(can_gc)
        .and_then(|selection| selection.GetRangeAt(0).ok())
}

/// <https://w3c.github.io/editing/docs/execCommand/#formattable-node>
fn is_formattable_node(node: &Node) -> bool {
    let is_correct_type = matches!(
        node.type_id(),
        NodeTypeId::CharacterData(CharacterDataTypeId::Text(TextTypeId::Text)) |
            NodeTypeId::Element(ElementTypeId::HTMLElement(
                HTMLElementTypeId::HTMLImageElement | HTMLElementTypeId::HTMLBRElement
            ))
    );

    // FIXME: Check if the node is visible
    is_correct_type && node.is_editable()
}

fn effectively_contained_nodes(range: &Range) -> impl Iterator<Item=DomRoot<Node>> {
    let effectively_contained_nodes = if range.collapsed() {
        None
    } else {
        // Traversing all of these nodes is necessary because even nodes which are not contained
        // in the range can be effectively contained.
        let iter = range.common_ancestor_container()
            .traverse_preorder(ShadowIncluding::No)
            .filter(|node| is_effectively_contained_in_range(node, range));
        Some(iter)
    };

    effectively_contained_nodes.into_iter()
}

/// <https://w3c.github.io/editing/docs/execCommand/#effectively-contained>
fn is_effectively_contained_in_range(node: &Node, range: &Range) -> bool {
    // A node node is effectively contained in a range range if range is not collapsed,
    // and at least one of the following holds:
    // NOTE: for_each_effectively_contained_node already checks whether the range is collapsed
    debug_assert!(!range.collapsed());

    // * node is contained in range.
    if range.contains(node) {
        return true;
    }

    // * node is range's start node, it is a Text node, and its length is different from range's start offset.
    if node == range.start_container() && node.is::<Text>() && node.len() != range.start_offset() {
        return true;
    }

    // * node is range's end node, it is a Text node, and range's end offset is not 0.
    if node == range.end_container() && range.end_offset() != 0 {
        return true;
    }

    // * node has at least one child; and all its children are effectively contained in range;
    // and either range's start node is not a descendant of node or is not a Text node or range's
    // start offset is zero; and either range's end node is not a descendant of node or is not a
    // Text node or range's end offset is its end node's length.
    if node.children_count() != 0 && node.children().all(|child| is_effectively_contained_in_range(&child, range)) {
        let start_condition = node.is_ancestor_of(&range.start_container()) || !node.is::<Text>() || range.start_offset() == 0;
        let end_container = range.end_container();
        let end_condition = node.is_ancestor_of(&end_container) || !node.is::<Text>() || range.end_offset() == end_container.len();
        if start_condition && end_condition {
            return true;
        }
    }

    false
}

/// <https://w3c.github.io/editing/docs/execCommand/#clear-the-value>
fn clear_the_value_of(element: &Element, command: Command) -> Vec<()> {
    // Step 1. Let command be the current command.

    // Step 2. If element is not editable, return the empty list.
    if !element.upcast::<Node>().is_editable() {
        return vec![];
    }

    // Step 3. If element's specified command value for command is null, return the empty list.
    let specified_value =
}

/// <https://w3c.github.io/editing/docs/execCommand/#specified-command-value>
fn specified_command_value(element: &Element, command: Command) {
    // Step 1. If command is "backColor" or "hiliteColor" and the Element's display
    // property does not have resolved value "inline", return null.
    match command {
        Command::BackColor
    }
}
