/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use js::context::JSContext;
use script_bindings::codegen::GenericBindings::NodeBinding::NodeMethods;
use style::color::AbsoluteColor;
use markup5ever::local_name;

use crate::dom::Element;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::str::{DOMString, FromInputValueString};
use crate::dom::document::Document;
use crate::dom::execcommand::basecommand::CommandName;
use crate::dom::execcommand::contenteditable::node::{NodeOrString, is_allowed_child, record_the_values};
use crate::dom::iterators::ShadowIncluding;
use crate::dom::selection::Selection;
use crate::dom::types::HTMLElement;

/// <https://w3c.github.io/editing/docs/execCommand/#the-formatblock-command>
pub(crate) fn execute_formatblock_command(
    cx: &mut JSContext,
    document: &Document,
    selection: &Selection,
    value: DOMString,
) -> bool {
    // Step 1. If value begins with a "<" character and ends with a ">" character, remove the first
    // and last characters from it.
    let value: &str = &value.str();
    let value = value.strip_prefix("<").unwrap_or(value);
    let value = value.strip_suffix(">").unwrap_or(value);

    // Step 2. Let value be converted to ASCII lowercase.
    let value = value.to_ascii_lowercase();

    // Step 3.  If value is not a formattable block name, return false.
    if !is_formattable_block_name(&value) {
        return false;
    }

    // Step 4. Block-extend the active range, and let new range be the result.
    let new_range = selection
        .active_range()
        .expect("Must always have an active range")
        .block_extend(cx, document);

    // Step 5. Let node list be an empty list of nodes.
    // Step 6. For each node node contained in new range, append node to node list if it is editable,
    // the last member of original node list (if any) is not an ancestor of node, node is either a
    // non-list single-line container or an allowed child of "p" or a dd or dt, and node is not the
    // ancestor of a prohibited paragraph child.
    let mut node_list = vec![];
    let range_root = new_range.start_container().GetRootNode(&Default::default());
    let end_container = new_range.end_container();
    for node in new_range
        .start_container()
        .following_nodes(&range_root, ShadowIncluding::No)
    {
        if node_list.is_empty() {
            // If this is the start container of the range then it may not be fully contained in the range in which
            // case we want to ignore it.
            if !range_root.Contains(Some(&node)) {
                continue;
            }
        }

        // Similarly we only want to include the end container if its fully contained in the range.
        if node == end_container {
            if !range_root.Contains(Some(&node)) {
                break;
            }
        }

        // > if it is editable,
        if !node.is_editable() {
            continue;
        }

        if let Some(element) = node.downcast::<Element>() {
            if !element.is_non_list_single_line_container() &&
                !is_allowed_child(NodeOrString::Node(node), NodeOrString::String("p".to_owned())) &&
                !matches!(*element.local_name(), local_name!("dd") | local_name!("dt"))
            {
                continue;
            }
        }

        // > and node is not the ancestor of a prohibited paragraph child.
        if node.traverse_preorder_non_rooting(cx.no_gc(), ShadowIncluding::No).skip(1).any(|descendant| descendant.is_prohibited_paragraph_child()) {
            continue;
        }

        node_list.push(node);
    }

    // Step 7. Record the values of node list, and let values be the result.
    let values = record_the_values(node_list);

    // Step 8. For each node in node list, while node is the descendant of an editable HTML element in the same editing host, whose local name is a formattable block name, and which is not the ancestor of a prohibited paragraph child, split the parent of the one-node list consisting of node.

    true
}

/// <https://w3c.github.io/editing/docs/execCommand/#formattable-block-name>
fn is_formattable_block_name(value: &str) -> bool {
    matches!(
        value,
        "address" | "dd" | "div" | "dt" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "pre"
    )
}
