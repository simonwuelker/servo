/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::rust::HandleObject;
use mime::Mime;

use crate::dom::audiotracklist::AudioTrackList;
use crate::dom::bindings::codegen::Bindings::MediaSourceBinding::MediaSourceMethods;
use crate::dom::bindings::error::{Error, Fallible};
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::reflector::reflect_dom_object_with_proto;
use crate::dom::bindings::root::{DomRoot, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::dom::mediasourcehandle::MediaSourceHandle;
use crate::dom::sourcebuffer::SourceBuffer;
use crate::dom::sourcebufferlist::SourceBufferList;
use crate::dom::window::Window;
use crate::script_runtime::CanGc;

/// <https://w3c.github.io/media-source/#mediasource>
#[dom_struct]
pub struct MediaSource {
    eventtarget: EventTarget,
    source_buffer_list: MutNullableDom<SourceBufferList>,

    /// <https://w3c.github.io/media-source/#dom-mediasource-handle>
    handle: MutNullableDom<MediaSourceHandle>,
}

impl MediaSource {
    pub fn new_inherited() -> MediaSource {
        Self {
            eventtarget: EventTarget::new_inherited(),
            source_buffer_list: MutNullableDom::new(None),
            handle: MutNullableDom::new(None),
        }
    }

    pub fn new(global: &GlobalScope, can_gc: CanGc) -> DomRoot<MediaSource> {
        Self::new_with_proto(global, None, can_gc)
    }

    fn new_with_proto(
        global: &GlobalScope,
        proto: Option<HandleObject>,
        can_gc: CanGc,
    ) -> DomRoot<MediaSource> {
        reflect_dom_object_with_proto(
            Box::new(MediaSource::new_inherited()),
            global,
            proto,
            can_gc,
        )
    }

    fn get_or_init_src_buffer(&self) -> DomRoot<SourceBufferList> {
        let global_object = GlobalScope::current().expect("No current global object");

        self.source_buffer_list
            .or_init(|| SourceBufferList::new(&*global_object, CanGc::note(), &[]))
    }
}

impl MediaSourceMethods<crate::DomTypeHolder> for MediaSource {
    /// <https://w3c.github.io/media-source/#dom-mediasource-constructor>
    fn Constructor(
        window: &Window,
        proto: Option<HandleObject>,
        can_gc: CanGc,
    ) -> DomRoot<MediaSource> {
        MediaSource::new_with_proto(window.upcast::<GlobalScope>(), proto, can_gc)
    }

    /// <https://w3c.github.io/media-source/#dom-mediasource-handle>
    fn Handle(&self) -> DomRoot<MediaSourceHandle> {
        let global_object = GlobalScope::current().expect("No current global object");

        self.handle
            .or_init(|| MediaSourceHandle::new(&*global_object, self))
    }

    /// <https://w3c.github.io/media-source/#dom-mediasource-istypesupported>
    fn IsTypeSupported(_window: &Window, media_type: DOMString) -> bool {
        // Step 1. If type is an empty string, then return false.
        if media_type.is_empty() {
            return false;
        }

        // Step 2. If type does not contain a valid MIME type string, then return false.
        let Ok(_mime_type) = media_type.str().parse::<Mime>() else {
            return false;
        };

        // TODO Step 3. If type contains a media type or media subtype that the
        // MediaSource does not support, then return false.

        // TODO Step 4. If type contains a codec that the MediaSource does not support, then return false.

        // TODO Step 5. If the MediaSource does not support the specified combination of media type,
        // media subtype, and codecs then return false.

        // Step 6. Return true.
        true
    }

    /// <https://w3c.github.io/media-source/#addsourcebuffer-method>
    fn AddSourceBuffer(&self, buffer_type: DOMString) -> Fallible<DomRoot<SourceBuffer>> {
        // Step 1. If type is an empty string then throw a TypeError exception and abort these steps.
        if buffer_type.is_empty() {
            return Err(Error::Type("type must not be empty".to_owned()));
        }

        // Step 2. If type contains a MIME type that is not supported or contains a MIME type that is
        // not supported with the types specified for the other SourceBuffer objects in sourceBuffers,
        // then throw a NotSupportedError exception and abort these steps.
        let Ok(_mime_type) = buffer_type.str().parse::<Mime>() else {
            // TODO: the spec doesn't explicitly state what happens in this case
            return Err(Error::Type("invalid mime type".to_owned()));
        };

        // TODO Step 3. If the user agent can't handle any more SourceBuffer objects or if creating a
        // SourceBuffer based on type would result in an unsupported SourceBuffer configuration,
        // then throw a QuotaExceededError exception and abort these steps.

        // TODO Step 4. If the readyState attribute is not in the "open" state then throw an
        // InvalidStateError exception and abort these steps.

        // Step 5. Let buffer be a new instance of a ManagedSourceBuffer if this is a ManagedMediaSource,
        // or a SourceBuffer otherwise, with their respective associated resources.
        //
        let global_object = GlobalScope::current().expect("No current global object");
        let audio_track_list = AudioTrackList::new(
            global_object.downcast::<Window>().expect("is not a window"),
            &[],
            None,
            CanGc::note(),
        );
        let buffer = SourceBuffer::new(&*global_object, CanGc::note(), &*audio_track_list);

        // TODO Step 6. Set buffer's [[generate timestamps flag]] to the value in the "Generate Timestamps Flag"
        // column of the Media Source Extensions™ Byte Stream Format Registry entry that is associated with type.

        // TODO Step 7. If buffer's [[generate timestamps flag]] is true, set buffer's mode to "sequence".
        // Otherwise, set buffer's mode to "segments".

        // Step 8. Append buffer to this's sourceBuffers.
        self.get_or_init_src_buffer().push(&*buffer);

        // TODO Step 9. Queue a task to fire an event named addsourcebuffer at this's sourceBuffers.

        // Step 10. Return buffer.
        Ok(buffer)
    }

    /// <https://w3c.github.io/media-source/#dom-mediasource-sourcebuffers>
    fn SourceBuffers(&self) -> DomRoot<SourceBufferList> {
        self.get_or_init_src_buffer()
    }

    // https://w3c.github.io/media-source/#dom-mediasource-onsourceopen
    event_handler!(sourceopen, GetOnsourceopen, SetOnsourceopen);

    // https://w3c.github.io/media-source/#dom-mediasource-onsourceended
    event_handler!(sourceended, GetOnsourceended, SetOnsourceended);

    // https://w3c.github.io/media-source/#dom-mediasource-onsourceclose
    event_handler!(sourceclose, GetOnsourceclose, SetOnsourceclose);
}
