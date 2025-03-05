/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use style_dom::ElementState;
use script_bindings::root::DomRoot;
use js::rust::HandleObject;
use html5ever::{ns, LocalName, Prefix, namespace_url};

use crate::dom::bindings::codegen::GenericBindings::EventHandlerBinding::EventHandlerNonNull;
use crate::dom::bindings::codegen::Bindings::MathMLElementBinding::MathMLElementMethods;
use crate::dom::document::Document;
use crate::dom::element::Element;
use crate::script_runtime::CanGc;
use crate::dom::node::Node;

#[dom_struct]
pub(crate) struct MathMLElement {
    element: Element,
}


impl MathMLElement {
    pub(crate) fn new_inherited(
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> MathMLElement {
        MathMLElement::new_inherited_with_state(ElementState::empty(), tag_name, prefix, document)
    }

    pub(crate) fn new_inherited_with_state(
        state: ElementState,
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> MathMLElement {
        MathMLElement {
            element: Element::new_inherited_with_state(
                state,
                tag_name,
                ns!(mathml),
                prefix,
                document,
            ),
        }
    }

    #[cfg_attr(crown, allow(crown::unrooted_must_root))]
    pub(crate) fn new(
        local_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
        can_gc: CanGc,
    ) -> DomRoot<MathMLElement> {
        Node::reflect_node_with_proto(
            Box::new(MathMLElement::new_inherited(local_name, prefix, document)),
            document,
            proto,
            can_gc,
        )
    }
}

impl MathMLElementMethods<crate::DomTypeHolder> for MathMLElement {
    // https://html.spec.whatwg.org/multipage/#globaleventhandlers
    global_event_handlers!(NoOnload);
}