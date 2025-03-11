/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use js::rust::HandleObject;

use crate::dom::bindings::root::DomRoot;
use crate::dom::document::Document;
use crate::dom::node::Node;
use crate::dom::svggeometryelement::SVGGeometryElement;
use crate::script_runtime::CanGc;

#[dom_struct]
pub(crate) struct SVGCircleElement {
    svggeometryelement: SVGGeometryElement,
}

impl SVGCircleElement {
    pub(crate) fn new_inherited(
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGCircleElement {
        SVGCircleElement {
            svggeometryelement: SVGGeometryElement::new_inherited(tag_name, prefix, document),
        }
    }

    pub(crate) fn new(
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
        proto: Option<HandleObject>,
        can_gc: CanGc,
    ) -> DomRoot<SVGCircleElement> {
        Node::reflect_node_with_proto(
            Box::new(SVGCircleElement::new_inherited(tag_name, prefix, document)),
            document,
            proto,
            can_gc,
        )
    }
}
