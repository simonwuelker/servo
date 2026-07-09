/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use js::context::{JSContext, NoGC};
use markup5ever::local_name;
use script_bindings::codegen::GenericBindings::ElementBinding::ElementMethods;
use script_bindings::codegen::GenericBindings::NodeBinding::NodeMethods;
use script_bindings::root::DomRoot;

use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::str::{DOMString};
use crate::dom::document::Document;
use crate::dom::execcommand::contenteditable::node::{
    NodeOrString, is_allowed_child, record_the_values, restore_the_values, split_the_parent,
    wrap_node_list,
};
use crate::dom::iterators::ShadowIncluding;
use crate::dom::selection::Selection;
use crate::dom::types::{HTMLBRElement, HTMLElement};
use crate::dom::{CharacterData, Element, Node};

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
        let is_end_container = node == end_container;
        if is_end_container {
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
                !is_allowed_child(
                    NodeOrString::Node(node.clone()),
                    NodeOrString::String("p".to_owned()),
                ) &&
                !matches!(*element.local_name(), local_name!("dd") | local_name!("dt"))
            {
                continue;
            }
        }

        // > and node is not the ancestor of a prohibited paragraph child.
        if node
            .traverse_preorder_non_rooting(cx.no_gc(), ShadowIncluding::No)
            .skip(1)
            .any(|descendant| descendant.is_prohibited_paragraph_child())
        {
            continue;
        }

        node_list.push(node);

        if is_end_container {
            break;
        }
    }

    // Step 7. Record the values of node list, and let values be the result.
    let values = record_the_values(&node_list);

    // Step 8. For each node in node list, while node is the descendant of an editable HTML element in the same
    // editing host, whose local name is a formattable block name, and which is not the ancestor of a prohibited
    // paragraph child, split the parent of the one-node list consisting of node.
    for node in &node_list {
        let should_continue_looping = |no_gc: &NoGC, node: &Node| -> bool {
            node.inclusive_ancestors_unrooted(no_gc, ShadowIncluding::No)
                .filter(|ancestor| ancestor.is::<HTMLElement>())
                .filter(|ancestor| ancestor.is_editable())
                .filter(|ancestor| ancestor.same_editing_host(node))
                .filter(|ancestor| {
                    !ancestor
                        .traverse_preorder_non_rooting(no_gc, ShadowIncluding::No)
                        .skip(1)
                        .any(|descendant| descendant.is_prohibited_paragraph_child())
                })
                .next()
                .is_some()
        };

        while should_continue_looping(cx, &node) {
            split_the_parent(cx, &[&node]);
        }
    }

    // Step 9. Restore the values from values.
    restore_the_values(cx, values);

    // Step 10. While node list is not empty:
    // Note: The algorithm always ends up removing the first member, so we do that immediately.
    while !node_list.is_empty() {
        let mut sublist: Vec<DomRoot<Node>>;
        let first_member = node_list.remove(0);
        // Step 10.1 If the first member of node list is a single-line container:
        if first_member.is_single_line_container() {
            // Step 10.1.1  Let sublist be the children of the first member of node list.
            sublist = first_member.children().collect();

            // Step 10.2.2 Record the values of sublist, and let values be the result.
            let values = record_the_values(&sublist);

            // Step 10.2.3 Remove the first member of node list from its parent, preserving its descendants.
            first_member.remove_self(cx);

            // Step 10.2.4 Restore the values from values.
            restore_the_values(cx, values);

            // Step 10.2.5 Remove the first member from node list.
            node_list.remove(0);
        }
        // Step 10.2 Otherwise:
        else {
            // Step 10.2.1 Let sublist be an empty list of nodes.
            // Step 10.2.2 Remove the first member of node list and append it to sublist.
            sublist = vec![first_member];

            // Step 10.2.3 While node list is not empty, and the first member of node list is the nextSibling
            // of the last member of sublist, and the first member of node list is not a single-line container,
            // and the last member of sublist is not a br, remove the first member of node list and append it
            // to sublist.
            while let Some(first_member_of_node_list) = node_list.first() {
                let last_member_of_sublist = sublist.last().expect("sublist can never be empty");
                let Some(next_sibling_of_last_member_of_sublist) =
                    last_member_of_sublist.GetNextSibling()
                else {
                    break;
                };
                if *first_member_of_node_list != next_sibling_of_last_member_of_sublist {
                    break;
                }
                if first_member_of_node_list.is_single_line_container() {
                    break;
                }
                if last_member_of_sublist.is::<HTMLBRElement>() {
                    break;
                }
                sublist.push(node_list.remove(0));
            }
        }

        // Step 10.3 Wrap sublist. If value is "div" or "p", sibling criteria returns false; otherwise it returns true
        // for an HTML element with local name value and no attributes, and false otherwise. New parent instructions
        // return the result of running createElement(value) on the context object. Then fix disallowed ancestors
        // of the result.
        let result = wrap_node_list(
            cx,
            sublist.clone(),
            |sibling| {
                if matches!(value.as_str(), "div" | "p") {
                    return false;
                } else {
                    sibling.downcast::<Element>().is_some_and(|sibling| {
                        sibling.is::<HTMLElement>() && !sibling.HasAttributes()
                    })
                }
            },
            |cx| {
                Some(DomRoot::upcast(document.create_element(cx, &value)))
            },
        );
        if let Some(result) = result {
            result.fix_disallowed_ancestors(cx, document);
        }
    }

    // Step 11. Return true.
    true
}

/// <https://w3c.github.io/editing/docs/execCommand/#formattable-block-name>
fn is_formattable_block_name(value: &str) -> bool {
    matches!(
        value,
        "address" | "dd" | "div" | "dt" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "pre"
    )
}
