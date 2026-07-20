/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::Cell;

use dom_struct::dom_struct;
use js::context::{JSContext, NoGC};
use js::rust::HandleObject;
use layout_api::{ReflowRequest, with_layout_state};
use script_bindings::codegen::GenericBindings::DocumentBinding::DocumentMethods;
use script_bindings::codegen::GenericBindings::WindowBinding::WindowMethods;
use script_bindings::inheritance::Castable;
use script_bindings::reflector::reflect_dom_object_with_proto;
use script_bindings::root::{Dom, DomRoot};
use style::stylesheets::keyframes_rule::{KeyframesAnimation, KeyframesStep};

use crate::dom::animationeffect::AnimationEffect;
use crate::dom::bindings::codegen::Bindings::AnimationBinding::AnimationMethods;
use crate::dom::bindings::root::{MutNullableDom, ToLayout};
use crate::dom::eventtarget::EventTarget;
use crate::dom::globalscope::GlobalScope;
use crate::dom::node::Node;
use crate::dom::types::{AnimationTimeline, KeyframeEffect};
use crate::dom::window::Window;
use crate::dom::{Document, NodeTraits};

/// <https://drafts.csswg.org/web-animations-1/#animation>
#[dom_struct]
pub(crate) struct Animation {
    event_target: EventTarget,

    /// <https://drafts.csswg.org/web-animations-1/#timeline>
    timeline: MutNullableDom<AnimationTimeline>,

    /// <https://drafts.csswg.org/web-animations-1/#animation-associated-effect>
    associated_effect: MutNullableDom<AnimationEffect>,

    /// <https://drafts.csswg.org/web-animations-1/#animation-start-time>
    start_time: Cell<PotentiallyUnresolvedTimeValue>,
}

/// <https://drafts.csswg.org/web-animations-1/#time-value>
#[derive(Clone, Copy, Debug, JSTraceable, MallocSizeOf)]
pub(crate) enum PotentiallyUnresolvedTimeValue {
    Resolved(f64),
    Unresolved,
}

impl Animation {
    pub(crate) fn new_inherited() -> Self {
        Self {
            event_target: EventTarget::new_inherited(),
            timeline: Default::default(),
            associated_effect: Default::default(),
            start_time: Cell::new(PotentiallyUnresolvedTimeValue::Unresolved),
        }
    }

    fn new_with_proto_and_cx(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
    ) -> DomRoot<Self> {
        reflect_dom_object_with_proto(cx, Box::new(Self::new_inherited()), global, proto)
    }

    pub(crate) fn new(cx: &mut JSContext, global: &GlobalScope) -> DomRoot<Self> {
        Self::new_with_proto_and_cx(cx, global, None)
    }

    /// <https://drafts.csswg.org/web-animations-1/#play-an-animation>
    #[cfg_attr(crown, expect(crown::unrooted_must_root))]
    #[expect(unsafe_code)]
    pub(crate) fn play(&self, document: &Document) {
        // FIXME: Implement this according to the specification

        let Some(effect) = self.associated_effect.get() else {
            return;
        };

        // This needs to be changed if the specification ever defines animation effects that are not
        // keyframe effects.
        let Some(keyframeeffect) = effect.downcast::<KeyframeEffect>() else {
            return;
        };
        let Some(target_element) = keyframeeffect.target_element() else {
            return;
        };

        let layout = target_element.owner_window().layout();
        let x = KeyframesAnimation {
            steps: todo!(),
            steps_with_range_name: Vec::default(),
        };

        // SAFETY: These are unrooted, but kept alive by the roots that we create them from
        let traced_node = Dom::from_ref(self);

        with_layout_state(|| {
            let layout_node = unsafe { traced_node.to_layout() };
            layout.start_animation_from_script(target_element.upcast::<Node>(), ReflowRequest {});
        });
    }

    /// <https://drafts.csswg.org/web-animations-1/#animation-set-the-timeline-of-an-animation>
    fn set_the_timeline(&self, timeline: &AnimationTimeline) {
        // FIXME: Implement this fully
        self.timeline.set(Some(timeline));
    }

    /// <https://drafts.csswg.org/web-animations-1/#animation-set-the-associated-effect-of-an-animation>
    fn set_the_associated_effect(&self, effect: Option<&AnimationEffect>) {
        // FIXME: Implement this fully
        self.associated_effect.set(effect);
    }

    /// <https://drafts.csswg.org/web-animations-1/#the-current-time-of-an-animation>
    fn current_time(&self, no_gc: &NoGC) -> PotentiallyUnresolvedTimeValue {
        // > The current time of an animation is calculated from the first matching condition below:
        // > If the animation’s hold time is resolved,
        // Note: We don't implement hold time yet, so this is never true

        // > If any of the following are true:
        // > * the animation has no associated timeline, or
        // > * the associated timeline is inactive, or
        // > * the animation’s start time is unresolved,
        // > The current time is an unresolved time value.
        let Some(timeline) = self.timeline.get_unrooted(no_gc) else {
            return PotentiallyUnresolvedTimeValue::Unresolved;
        };
        // Otherwise,
        // > current time = (timeline time − start time) × playback rate
        todo!()
    }

    /// <https://drafts.csswg.org/web-animations-1/#play-an-animation>
    fn animate(&self) {
        // Step 4. If the auto-rewind flag is true, perform the steps corresponding to the first matching condition from
        // the following, if any:
        // Set seek time to zero.
        let seek_time = 0.0;
    }
}

impl AnimationMethods<crate::DomTypeHolder> for Animation {
    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-animation>
    fn Constructor(
        cx: &mut JSContext,
        window: &Window,
        _object: Option<HandleObject>,
        effect: Option<&AnimationEffect>,
    ) -> DomRoot<Self> {
        let document = window.Document();

        // Step 1. Let animation be a new Animation object.
        let animation = Animation::new(cx, window.upcast());

        // Step 2. Run the procedure to set the timeline of an animation on animation passing timeline
        // as the new timeline; or, if the timeline argument is missing, passing the default document
        // timeline of the Document associated with the Window that is the current global object.
        // TODO: We don't suppor the timeline argument yet.
        animation.set_the_timeline(document.Timeline().upcast());

        // Step 3. Run the procedure to set the associated effect of an animation on animation passing
        // source as the new effect.
        animation.set_the_associated_effect(effect);

        animation
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-effect>
    fn GetEffect(&self) -> Option<DomRoot<AnimationEffect>> {
        self.associated_effect.get()
    }

    /// <https://drafts.csswg.org/web-animations-1/#dom-animation-effect>
    fn SetEffect(&self, effect: Option<&AnimationEffect>) {
        // > Setting this attribute updates the object’s associated effect using
        //> the procedure to set the associated effect of an animation.
        self.set_the_associated_effect(effect);
    }
}
