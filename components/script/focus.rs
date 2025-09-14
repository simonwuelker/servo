use script_bindings::codegen::GenericBindings::HTMLOrSVGElementBinding::FocusOptions;
use script_bindings::root::DomRoot;
use script_bindings::script_runtime::CanGc;

use crate::dom::bindings::codegen::Bindings::ShadowRootBinding::ShadowRootMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::document::{Document, FocusInitiator};
use crate::dom::element::Element;
use crate::dom::htmldialogelement::HTMLDialogElement;
use crate::dom::node::{Node, NodeTraits, ShadowIncluding};

// TODO: This module currently assumes that the term "focusable area" is equal to an Element.
// That is not correct. For example, a focusable area can be the viewport or a shape of an
// <area> element.
pub(crate) type FocusableArea = Element;

/// <https://html.spec.whatwg.org/multipage/#focusing-steps>
pub(crate) fn run_focusing_steps(
    document: &Document,
    new_focus_target: Option<&Element>,
    focus_initiator: FocusInitiator,
    focus_options: FocusOptions,
    can_gc: CanGc,
) {
    // Step 1. If new focus target is not a focusable area, then set new focus target to the result of getting
    // the focusable area for new focus target, given focus trigger if it was passed.
    let new_focus_target = new_focus_target.and_then(|focus_target| {
        if focus_target.is_focusable_area() {
            Some(DomRoot::from_ref(focus_target))
        } else {
            get_the_focusable_area(focus_target)
        }
    });

    // Step 2. If new focus target is null, then:
    let Some(new_focus_target) = new_focus_target else {
        // Step 2.1 If no fallback target was specified, then return.
        // Step 2.2 Otherwise, set new focus target to the fallback target.
        // NOTE: We don't support fallback targets yet.
        return;
    };

    // TODO: Step 3. If new focus target is a navigable container with non-null content navigable,
    // then set new focus target to the content navigable's active document.

    // TODO: Step 4. If new focus target is a focusable area and its DOM anchor is inert, then return.

    // Step 5. If new focus target is the currently focused area of a top-level traversable, then return.
    // FIXME: The document is not necessarily the top level traversable
    let currently_focused_area = document.get_focused_element();
    if currently_focused_area.is_some_and(|focused_area| focused_area == new_focus_target) {
        return;
    }

    // Step 6. Let old chain be the current focus chain of the top-level traversable in which new focus target finds itself.
    // NOTE: The spec doesn't really specify what the current focus chain is if there's no focused element. We use an empty Vec.
    let old_chain = currently_focused_area.map(construct_focus_chain_for_focusable_area).unwrap_or_default();

    // Step 7. Let new chain be the focus chain of new focus target.
    let new_chain = construct_focus_chain_for_focusable_area(&new_focus_target);

    // Step 8. Run the focus update steps with old chain, new chain, and new focus target respectively.


    document.request_focus_with_options(
        Some(&new_focus_target),
        focus_initiator,
        focus_options,
        can_gc,
    );
}

/// <https://html.spec.whatwg.org/multipage/interaction.html#focus-update-steps>
fn run_focus_update_steps(mut old_chain: Vec<DomRoot<Node>>, mut new_chain: Vec<DomRoot<Node>>) {
    loop {
        // Step 1. If the last entry in old chain and the last entry in new chain are the same,
        // pop the last entry from old chain and the last entry from new chain and redo this step.
        if old_chain.last().zip(new_chain.last()).is_some_and(|(old_entry, new_entry)| old_entry == new_entry) {
            old_chain.pop();
            new_chain.pop();
        }
    }

    // Step 2. For each entry entry in old chain, in order, run these substeps:
    for entry in &old_chain {
        // Step 2.1 If entry is an input element, and the change event applies to the element [..]

        // Step 2.2 If entry is an element, let blur event target be entry.
        let blur_event_target = if entry.is::<Element>() {
            Some(DomRoot::from_ref(entry.upcast::<EventTarget>()))
        }
        // If entry is a Document object, let blur event target be that Document object's relevant global object.
        else if entry.is::<Document>() {
            Some(DomRoot::upcast::<EventTarget>(entry.owner_window()))
        }
        // Otherwise, let blur event target be null.
        else {
            None
        };

        // Step 2.3 If entry is the last entry in old chain, and entry is an Element, and the last entry in new chain
        // is also an Element, then let related blur target be the last entry in new chain.
        // Otherwise, let related blur target be null.
        let related_blur_target = new_chain.last()
            .filter(|last_entry_from_new_chain| old_chain.last().unwrap() == entry && last_entry_from_new_chain.is::<Element>())
            .map(|entry| entry.upcast::<EventTarget>())
            .map(DomRoot::from_ref);

        // Step 2.4 If blur event target is not null, fire a focus event named blur at blur event target, with related blur target as the related target.
    }
}

/// <https://html.spec.whatwg.org/multipage/#get-the-focusable-area>
fn get_the_focusable_area(focus_target: &Element) -> Option<DomRoot<FocusableArea>> {
    // TODO: If focus target is an area element with one or more shapes that are focusable areas
    // TODO: If focus target is an element with one or more scrollable regions that are focusable areas
    // TODO: If focus target is the document element of its Document
    // TODO: If focus target is a navigable
    // TODO: If focus target is a navigable container with a non-null content navigable
    // If focus target is a shadow host whose shadow root's delegates focus is true
    if focus_target
        .shadow_root()
        .is_some_and(|shadow_root| shadow_root.DelegatesFocus())
    {
        // TODO: Step 1. Let focusedElement be the currently focused area of a top-level traversable's DOM anchor.
        // TODO: Step 2. If focus target is a shadow-including inclusive ancestor of focusedElement, then return focusedElement.
        // Step 3. Return the focus delegate for focus target given focus trigger.
        return focus_delegate(focus_target);
    }
    // Otherwise
    else {
        // Return null.
        None
    }
}

/// <https://html.spec.whatwg.org/multipage/interaction.html#focus-delegate>
fn focus_delegate(focus_target: &Element) -> Option<DomRoot<Element>> {
    // Step 1. If focusTarget is a shadow host and its shadow root's delegates focus is false, then return null.
    let shadow_root = focus_target.shadow_root();
    if shadow_root
        .as_ref()
        .is_some_and(|shadow_root| !shadow_root.DelegatesFocus())
    {
        return None;
    }

    // Step 2. Let whereToLook be focusTarget.
    let mut where_to_look = DomRoot::from_ref(focus_target.upcast::<Node>());

    // Step 3. If whereToLook is a shadow host, then set whereToLook to whereToLook's shadow root.
    if let Some(shadow_root) = shadow_root {
        where_to_look = DomRoot::upcast(shadow_root);
    }

    // TODO: Step 4. Let autofocusDelegate be the autofocus delegate for whereToLook given focusTrigger.
    // TODO: Step 5. If autofocusDelegate is not null, then return autofocusDelegate.

    // Step 6. For each descendant of whereToLook's descendants, in tree order:
    for descendant in where_to_look.traverse_preorder(ShadowIncluding::No) {
        // Step 6.1 Let focusableArea be null.
        let mut focusable_area = None;

        //  Step 6.2 If focusTarget is a dialog element and descendant is sequentially focusable,
        // then set focusableArea to descendant.
        if focus_target.is::<HTMLDialogElement>() {
            if let Some(sequentially_focusable_descendant) = descendant
                .downcast::<Element>()
                .filter(|element| element.is_sequentially_focusable())
            {
                focusable_area = Some(DomRoot::from_ref(sequentially_focusable_descendant));
            }
        }
        // Step 6.3 Otherwise, if focusTarget is not a dialog and descendant is a focusable area,
        // set focusableArea to descendant.
        else {
            if let Some(focusable_descendant) = descendant
                .downcast::<Element>()
                .map(DomRoot::from_ref)
                .filter(|element| element.is_focusable_area())
            {
                focusable_area = Some(focusable_descendant);
            }
        }

        // Step 6.4 Otherwise, set focusableArea to the result of getting the focusable area for descendant given focusTrigger.
        let focusable_area =
            focusable_area.or_else(|| get_the_focusable_area(descendant.downcast()?));

        // Step 6.5 If focusableArea is not null, then return focusableArea.
        if focusable_area.is_some() {
            return focusable_area;
        }
    }

    // Step 7. Return null.
    None
}

/// <https://html.spec.whatwg.org/multipage/#unfocusing-steps>
pub(crate) fn run_unfocusing_steps(document: &Document, can_gc: CanGc) {
    // TODO: Implement this
    document.request_focus_with_options(None, FocusInitiator::Local, Default::default(), can_gc);
}

/// <https://html.spec.whatwg.org/multipage/#focus-chain>
fn construct_focus_chain_for_focusable_area(subject: &FocusableArea) -> Vec<DomRoot<Node>> {
    // Step 1. Let output be an empty list.
    let mut output = vec![];

    // Step 2. Let currentObject be subject.
    let mut current_object: DomRoot<Node> = DomRoot::from_ref(subject.upcast());

    // Step 3. While true:
    loop {
        // Step 3.1 Append currentObject to output.
        output.push(current_object.clone());

        // TODO Step 3.2 If currentObject is an area element's shape, then append that area element to output.
        // Otherwise, if currentObject's DOM anchor is an element that is not currentObject itself,
        // then append currentObject's DOM anchor to output.

        // Step 3.2 If currentObject is a focusable area, then set currentObject to currentObject's DOM anchor's node document.
        // NOTE: The DOM anchor is the node itself since we only support focusable areas that are nodes.
        if current_object.downcast::<Element>().is_some_and(Element::is_focusable_area) {
            current_object = DomRoot::upcast(current_object.owner_document());
        }
        // TODO: Otherwise, if currentObject is a Document whose node navigable's parent is non-null,
        // then set currentObject to currentObject's node navigable's parent.
        // Otherwise, break.
        else {
            break;
        }
    }

    // Step 4. Return output.
    output
}
