/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use html5ever::{LocalName, Prefix};
use script_bindings::inheritance::Castable;

use crate::dom::document::Document;
use crate::dom::svggraphicselement::SVGGraphicsElement;
use crate::dom::virtualmethods::VirtualMethods;

/// <https://svgwg.org/svg2-draft/types.html#InterfaceSVGGeometryElement>
#[dom_struct]
pub(crate) struct SVGGeometryElement {
    svggraphicselement: SVGGraphicsElement,
}

impl SVGGeometryElement {
    pub(crate) fn new_inherited(
        tag_name: LocalName,
        prefix: Option<Prefix>,
        document: &Document,
    ) -> SVGGeometryElement {
        SVGGeometryElement {
            svggraphicselement: SVGGraphicsElement::new_inherited(tag_name, prefix, document),
        }
    }
}

impl VirtualMethods for SVGGeometryElement {
    fn super_type(&self) -> Option<&dyn VirtualMethods> {
        Some(self.upcast::<SVGGraphicsElement>() as &dyn VirtualMethods)
    }
}
