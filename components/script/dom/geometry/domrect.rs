/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use app_units::{AU_PER_PX, Au};
use dom_struct::dom_struct;
use euclid::Rect;
use js::context::{JSContext, NoGC};
use js::rust::HandleObject;
use rustc_hash::FxHashMap;
use script_bindings::reflector::{reflect_dom_object_with_cx, reflect_dom_object_with_proto};
use servo_base::id::{DomRectId, DomRectIndex};
use servo_constellation_traits::DomRect;
use style_traits::CSSPixel;

use crate::dom::bindings::codegen::Bindings::DOMRectBinding::DOMRectMethods;
use crate::dom::bindings::codegen::Bindings::DOMRectReadOnlyBinding::{
    DOMRectInit, DOMRectReadOnlyMethods,
};
use crate::dom::bindings::error::Fallible;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::serializable::Serializable;
use crate::dom::bindings::structuredclone::StructuredData;
use crate::dom::domrectreadonly::{DOMRectReadOnly, create_a_domrectreadonly_from_the_dictionary};
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub(crate) struct DOMRect {
    rect: DOMRectReadOnly,
}

impl DOMRect {
    fn new_inherited(x: f64, y: f64, width: f64, height: f64) -> DOMRect {
        DOMRect {
            rect: DOMRectReadOnly::new_inherited(x, y, width, height),
        }
    }

    pub(crate) fn new(
        cx: &mut JSContext,
        global: &GlobalScope,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> DomRoot<DOMRect> {
        Self::new_with_proto(cx, global, None, x, y, width, height)
    }

    fn new_with_proto(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> DomRoot<DOMRect> {
        reflect_dom_object_with_proto(
            cx,
            Box::new(DOMRect::new_inherited(x, y, width, height)),
            global,
            proto,
        )
    }

    /// Constructs a [DOMRect] and ensures that the edges of the rectangle can be
    /// added and subtracted without loss of precision.
    pub(crate) fn from_layout_rect(
        cx: &mut JSContext,
        global: &GlobalScope,
        layout_rect: Rect<Au, CSSPixel>,
    ) -> DomRoot<Self> {
        // We follow gecko's lead here:
        // https://searchfox.org/firefox-main/rev/d573d849c063353bda3a67541ab219c8a548773a/dom/base/DOMRect.cpp#148
        const SCALE: f64 = 65536.0; // 2^16
        const AU_TO_SCALED_PX: f64 = SCALE / (AU_PER_PX as f64);

        // We snap the Au values to a fine dyadic grid. This is important because it means
        // we can add, subtract and multiply the values on that grid without any floating point
        // imprecision. The error introduced by the snapping is at most half the size of a grid
        // cell, which is SCALE^-1. That is far smaller than one Au, so the loss of precision
        // is acceptable.
        let round_au_to_dyadic_grid =
            |value: Au| -> f64 { (value.0 as f64 * AU_TO_SCALED_PX).round() / SCALE };

        let snapped_left_edge = round_au_to_dyadic_grid(layout_rect.min_x());
        let snapped_top_edge = round_au_to_dyadic_grid(layout_rect.min_y());
        let snapped_right_edge = round_au_to_dyadic_grid(layout_rect.max_x());
        let snapped_bottom_edge = round_au_to_dyadic_grid(layout_rect.max_y());

        Self::new(
            cx,
            global,
            snapped_left_edge,
            snapped_top_edge,
            snapped_right_edge - snapped_left_edge,
            snapped_bottom_edge - snapped_top_edge,
        )
    }
}

impl DOMRectMethods<crate::DomTypeHolder> for DOMRect {
    /// <https://drafts.fxtf.org/geometry/#dom-domrect-domrect>
    fn Constructor(
        cx: &mut JSContext,
        global: &GlobalScope,
        proto: Option<HandleObject>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Fallible<DomRoot<DOMRect>> {
        Ok(DOMRect::new_with_proto(
            cx, global, proto, x, y, width, height,
        ))
    }

    // https://drafts.fxtf.org/geometry/#dom-domrect-fromrect
    #[cfg_attr(crown, expect(crown::unrooted_must_root))]
    fn FromRect(cx: &mut JSContext, global: &GlobalScope, other: &DOMRectInit) -> DomRoot<DOMRect> {
        let rect = create_a_domrectreadonly_from_the_dictionary(other);

        reflect_dom_object_with_cx(Box::new(Self { rect }), global, cx)
    }

    /// <https://drafts.fxtf.org/geometry/#dom-domrect-x>
    fn X(&self) -> f64 {
        self.rect.X()
    }

    /// <https://drafts.fxtf.org/geometry/#dom-domrect-x>
    fn SetX(&self, value: f64) {
        self.rect.set_x(value);
    }

    /// <https://drafts.fxtf.org/geometry/#dom-domrect-y>
    fn Y(&self) -> f64 {
        self.rect.Y()
    }

    /// <https://drafts.fxtf.org/geometry/#dom-domrect-y>
    fn SetY(&self, value: f64) {
        self.rect.set_y(value);
    }

    /// <https://drafts.fxtf.org/geometry/#dom-domrect-width>
    fn Width(&self) -> f64 {
        self.rect.Width()
    }

    /// <https://drafts.fxtf.org/geometry/#dom-domrect-width>
    fn SetWidth(&self, value: f64) {
        self.rect.set_width(value);
    }

    /// <https://drafts.fxtf.org/geometry/#dom-domrect-height>
    fn Height(&self) -> f64 {
        self.rect.Height()
    }

    /// <https://drafts.fxtf.org/geometry/#dom-domrect-height>
    fn SetHeight(&self, value: f64) {
        self.rect.set_height(value);
    }
}

impl Serializable for DOMRect {
    type Index = DomRectIndex;
    type Data = DomRect;

    fn serialize(&self, _no_gc: &NoGC) -> Result<(DomRectId, Self::Data), ()> {
        let serialized = DomRect {
            x: self.X(),
            y: self.Y(),
            width: self.Width(),
            height: self.Height(),
        };
        Ok((DomRectId::new(), serialized))
    }

    fn deserialize(
        cx: &mut JSContext,
        owner: &GlobalScope,
        serialized: Self::Data,
    ) -> Result<DomRoot<Self>, ()>
    where
        Self: Sized,
    {
        Ok(Self::new(
            cx,
            owner,
            serialized.x,
            serialized.y,
            serialized.width,
            serialized.height,
        ))
    }

    fn serialized_storage<'a>(
        data: StructuredData<'a, '_>,
    ) -> &'a mut Option<FxHashMap<DomRectId, Self::Data>> {
        match data {
            StructuredData::Reader(reader) => &mut reader.rects,
            StructuredData::Writer(writer) => &mut writer.rects,
        }
    }
}
