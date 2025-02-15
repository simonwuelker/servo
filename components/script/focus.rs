use crate::dom::bindings::root::Dom;
use crate::dom::element::Element;

/// <https://html.spec.whatwg.org/multipage/interaction.html#focus-chain>
struct FocusChain {
    elements: Vec<Dom<Element>>,
}

impl FocusChain {
    /// <https://html.spec.whatwg.org/multipage/interaction.html#focus-chain>
    fn for_target(subject: &Element) -> Self {
        // Step 1. Let output be an empty list.
        rooted_vec!(let output);

        // Step 2. Let currentObject be subject.
        let current_object = subject;

        // Step 3. While true:
        loop {
            // Step 3.1 Append currentObject to output.
            output.push(Dom::from_ref(current_object));

            // TODO Step 3.2 If currentObject is an area element's shape, then append that area element to output.
        }

        // Step 4. Return output.
        output
    }
}

/// <https://html.spec.whatwg.org/multipage/interaction.html#focusing-steps>
fn focusing_steps(new_focus_target: Option<&Element>, fallback_target: Option<&Element>) {
    // TODO: Step 1. If new focus target is not a focusable area, then set new
    // focus target to the result of getting the focusable area for new focus
    // target, given focus trigger if it was passed.

    // Step 2. If new focus target is null, then:
    let new_focus_target = match new_focus_target {
        Some(target) => target,
        None => {
            // Step 2.1 If no fallback target was specified, then return.
            let Some(fallback) = fallback_target else {
                return;
            };

            // Step 2.2 Otherwise, set new focus target to the fallback target.
            fallback
        }
    };

    // TODO: Step 3. If new focus target is a navigable container with non-null content navigable,
    // then set new focus target to the content navigable's active document.

    // TODO: Step 4. If new focus target is a focusable area and its DOM anchor is inert, then return.

    // TODO: Step 5. If new focus target is the currently focused area of a top-level traversable, then return.

    // TODO: Step 6. Let old chain be the current focus chain of the top-level traversable in which new focus target finds itself.

    // TODO: Step 7. Let new chain be the focus chain of new focus target.

    // TODO: Step 8. Run the focus update steps with old chain, new chain, and new focus target respectively.


}