/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;

use crate::dom::bindings::reflector::{reflect_dom_object, Reflector};
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::globalscope::GlobalScope;
use crate::dom::mediasource::MediaSource;
use crate::script_runtime::CanGc;

/// <https://w3c.github.io/media-source/#dom-mediasourcehandle>
#[dom_struct]
pub struct MediaSourceHandle {
    reflector: Reflector,

    /// <https://w3c.github.io/media-source/#dfn-has-ever-been-assigned-as-srcobject>
    has_ever_been_assigned_as_srcobject: Cell<bool>,

    /// The [MediaSource] object identified by this handle
    media_source: Dom<MediaSource>,
}

impl MediaSourceHandle {
    fn new_inherited(media_source: &MediaSource) -> MediaSourceHandle {
        Self {
            reflector: Reflector::new(),
            has_ever_been_assigned_as_srcobject: Cell::new(false),
            media_source: Dom::from_ref(media_source),
        }
    }

    pub fn new(global: &GlobalScope, media_source: &MediaSource) -> DomRoot<MediaSourceHandle> {
        reflect_dom_object(
            Box::new(MediaSourceHandle::new_inherited(media_source)),
            global,
            CanGc::note(),
        )
    }
}
