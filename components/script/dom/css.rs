/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use cssparser::{Parser, ParserInput, serialize_identifier};
use dom_struct::dom_struct;
use style::context::QuirksMode;
use style::parser::ParserContext;
use style::stylesheets::supports_rule::{Declaration, parse_condition_or_declaration};
use style::stylesheets::{CssRuleType, Origin, UrlExtraData};
use style_traits::ParsingMode;

use crate::dom::bindings::codegen::Bindings::CSSBinding::CSSMethods;
use crate::dom::bindings::codegen::Bindings::WindowBinding::Window_Binding::WindowMethods;
use crate::dom::bindings::error::Fallible;
use crate::dom::bindings::reflector::Reflector;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::window::Window;
use crate::dom::worklet::Worklet;

#[dom_struct]
#[allow(clippy::upper_case_acronyms)]
pub(crate) struct CSS {
    reflector_: Reflector,
}

impl CSSMethods<crate::DomTypeHolder> for CSS {
    /// <https://drafts.csswg.org/cssom/#the-css.escape()-method>
    fn Escape(_: &Window, ident: DOMString) -> Fallible<DOMString> {
        let mut escaped = String::new();
        serialize_identifier(&ident, &mut escaped).unwrap();
        Ok(DOMString::from(escaped))
    }

    /// <https://drafts.csswg.org/css-conditional/#dom-css-supports>
    fn Supports(win: &Window, property: DOMString, value: DOMString) -> bool {
        let mut decl = String::new();
        serialize_identifier(&property, &mut decl).unwrap();
        decl.push_str(": ");
        decl.push_str(&value);
        let decl = Declaration(decl);
        let url_data = UrlExtraData(win.Document().url().get_arc());
        let context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            /* namespaces = */ Default::default(),
            None,
            None,
        );
        decl.eval(&context)
    }

    /// <https://drafts.csswg.org/css-conditional/#dom-css-supports>
    fn Supports_(win: &Window, condition: DOMString) -> bool {
        let mut input = ParserInput::new(&condition);
        let mut input = Parser::new(&mut input);
        let cond = match parse_condition_or_declaration(&mut input) {
            Ok(c) => c,
            Err(..) => return false,
        };

        let url_data = UrlExtraData(win.Document().url().get_arc());
        let context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            /* namespaces = */ Default::default(),
            None,
            None,
        );
        cond.eval(&context)
    }

    /// <https://drafts.css-houdini.org/css-paint-api-1/#paint-worklet>
    fn PaintWorklet(win: &Window) -> DomRoot<Worklet> {
        win.paint_worklet()
    }
}
