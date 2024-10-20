/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;

use dom_struct::dom_struct;
use js::rust::HandleObject;

// use crate::dom::bindings::cell::RefCell;
use crate::dom::bindings::codegen::Bindings::SourceBufferListBinding::SourceBufferListMethods;
use crate::dom::bindings::reflector::reflect_dom_object_with_proto;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::dom::sourcebuffer::SourceBuffer;
use crate::script_runtime::CanGc;

/// <https://w3c.github.io/media-source/#dom-sourcebufferlist>
#[dom_struct]
pub struct SourceBufferList {
    eventtarget: EventTarget,
    source_buffers: RefCell<Vec<Dom<SourceBuffer>>>,
}

impl SourceBufferList {
    #[cfg_attr(crown, allow(crown::unrooted_must_root))]
    pub fn new_inherited(source_buffers: &[&SourceBuffer]) -> SourceBufferList {
        Self {
            eventtarget: EventTarget::new_inherited(),
            source_buffers: RefCell::new(
                source_buffers
                    .iter()
                    .map(|source_buffer| Dom::from_ref(&**source_buffer))
                    .collect(),
            ),
        }
    }

    pub fn new(
        global: &GlobalScope,
        can_gc: CanGc,
        source_buffers: &[&SourceBuffer],
    ) -> DomRoot<SourceBufferList> {
        Self::new_with_proto(global, None, can_gc, source_buffers)
    }

    fn new_with_proto(
        global: &GlobalScope,
        proto: Option<HandleObject>,
        can_gc: CanGc,
        source_buffers: &[&SourceBuffer],
    ) -> DomRoot<SourceBufferList> {
        reflect_dom_object_with_proto(
            Box::new(SourceBufferList::new_inherited(source_buffers)),
            global,
            proto,
            can_gc,
        )
    }

    #[cfg_attr(crown, allow(crown::unrooted_must_root))]
    pub fn push(&self, source_buffer: &SourceBuffer) {
        self.source_buffers
            .borrow_mut()
            .push(Dom::from_ref(source_buffer));
    }
}

impl SourceBufferListMethods<crate::DomTypeHolder> for SourceBufferList {
    /// <https://w3c.github.io/media-source/#dom-sourcebufferlist-length>
    fn Length(&self) -> u32 {
        self.source_buffers.borrow().len() as u32
    }
}
