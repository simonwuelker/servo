/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use cssparser::{Parser, ParserInput};
use dom_struct::dom_struct;
use servo_url::ServoUrl;

use crate::dom::bindings::codegen::Bindings::CSSStyleValueBinding::CSSStyleValueMethods;
use crate::dom::bindings::reflector::{reflect_dom_object, Reflector};
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::globalscope::GlobalScope;

#[dom_struct]
pub struct CSSStyleValue {
    reflector: Reflector,
    value: String,
}

impl CSSStyleValue {
    fn new_inherited(value: String) -> CSSStyleValue {
        CSSStyleValue {
            reflector: Reflector::new(),
            value,
        }
    }

    pub fn new(global: &GlobalScope, value: String) -> DomRoot<CSSStyleValue> {
        reflect_dom_object(Box::new(CSSStyleValue::new_inherited(value)), global)
    }
}

impl CSSStyleValueMethods for CSSStyleValue {
    /// <https://drafts.css-houdini.org/css-typed-om-1/#CSSStyleValue-stringification-behavior>
    fn Stringifier(&self) -> DOMString {
        DOMString::from(&*self.value)
    }
}
