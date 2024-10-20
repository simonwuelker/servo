/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::rust::HandleObject;

use crate::dom::audiotracklist::AudioTrackList;
use crate::dom::bindings::codegen::Bindings::SourceBufferBinding::SourceBufferMethods;
use crate::dom::bindings::reflector::reflect_dom_object_with_proto;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::script_runtime::CanGc;

/// <https://w3c.github.io/media-source/#dom-sourcebuffer>
#[dom_struct]
pub struct SourceBuffer {
    eventtarget: EventTarget,
    audio_tracks: Dom<AudioTrackList>,
}

impl SourceBuffer {
    pub fn new_inherited(audio_tracks: &AudioTrackList) -> SourceBuffer {
        Self {
            eventtarget: EventTarget::new_inherited(),
            audio_tracks: Dom::from_ref(audio_tracks),
        }
    }

    pub fn new(
        global: &GlobalScope,
        can_gc: CanGc,
        audio_tracks: &AudioTrackList,
    ) -> DomRoot<SourceBuffer> {
        Self::new_with_proto(global, None, can_gc, audio_tracks)
    }

    fn new_with_proto(
        global: &GlobalScope,
        proto: Option<HandleObject>,
        can_gc: CanGc,
        audio_tracks: &AudioTrackList,
    ) -> DomRoot<SourceBuffer> {
        reflect_dom_object_with_proto(
            Box::new(SourceBuffer::new_inherited(audio_tracks)),
            global,
            proto,
            can_gc,
        )
    }
}

impl SourceBufferMethods<crate::DomTypeHolder> for SourceBuffer {
    /// <https://w3c.github.io/media-source/#dom-sourcebuffer-audiotracks>
    fn AudioTracks(&self) -> DomRoot<AudioTrackList> {
        DomRoot::from_ref(&*self.audio_tracks)
    }
}
