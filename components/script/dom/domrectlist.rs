/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

 use dom_struct::dom_struct;

use crate::dom::bindings::codegen::Bindings::DOMRectListBinding::DOMRectListMethods;
use crate::dom::bindings::reflector::reflect_dom_object;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::domrect::DOMRect;
use crate::dom::window::Window;
use crate::Reflector;

/// <https://drafts.fxtf.org/geometry-1/#domrectlist>
#[dom_struct]
pub struct DOMRectList {
    reflector_: Reflector,
    elements: Vec<Dom<DOMRect>>,
}

impl DOMRectListMethods for DOMRectList {
    // https://drafts.fxtf.org/geometry-1/#dom-domrectlist-length
    fn Length(&self) -> u32 {
        // The length attribute must return the total number of DOMRect objects associated with the object.
        self.elements.len() as u32
    }

    // https://drafts.fxtf.org/geometry-1/#dom-domrectlist-item
    fn Item(&self, index: u32) -> Option<DomRoot<DOMRect>> {
        // The item(index) method, when invoked, must return null when index is greater than or equal to the
        // number of DOMRect objects associated with the DOMRectList. Otherwise, the DOMRect object at
        // index must be returned. Indices are zero-based.
        self.elements
            .get(index as usize)
            .map(|node| DomRoot::from_ref(&**node))
    }

    // https://drafts.fxtf.org/geometry-1/#dom-domrectlist-item
    fn IndexedGetter(&self, index: u32) -> Option<DomRoot<DOMRect>> {
        self.Item(index)
    }
}

impl DOMRectList {
    fn new_inherited(elements: impl Iterator<Item = DOMRect>) -> DOMRectList {
        DOMRectList {
            reflector_: Reflector::new(),
            elements: elements.map(|element| Dom::from_ref(&element)).collect(),
        }
    }

    pub fn new(window: &Window, elements: impl Iterator<Item = DOMRect>) -> DomRoot<DOMRectList> {
        reflect_dom_object(Box::new(DOMRectList::new_inherited(elements)), window)
    }

    pub fn elements(&self) -> &[Dom<DOMRect>] {
        &self.elements
    }
}
