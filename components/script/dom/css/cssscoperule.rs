/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::cell::RefCell;

use cssparser::ToCss;
use dom_struct::dom_struct;
use servo_arc::Arc;
use style::shared_lock::Locked;
use style::stylesheets::{CssRules, ScopeRule};
use style_traits::CssWriter;

use super::cssstylesheet::CSSStyleSheet;
use crate::dom::bindings::codegen::Bindings::CSSScopeRuleBinding::CSSScopeRuleMethods;
use crate::dom::bindings::reflector::reflect_dom_object;
use crate::dom::bindings::root::DomRoot;
use crate::dom::bindings::str::DOMString;
use crate::dom::css::cssgroupingrule::CSSGroupingRule;
use crate::dom::window::Window;
use crate::script_runtime::CanGc;

#[dom_struct]
pub(crate) struct CSSScopeRule {
    grouping_rule: CSSGroupingRule,
    #[no_trace]
    #[ignore_malloc_size_of = "Stylo"]
    scope_rule: RefCell<Arc<ScopeRule>>,
}

impl CSSScopeRule {
    fn new_inherited(
        parent_stylesheet: &CSSStyleSheet,
        scope_rule: Arc<ScopeRule>,
    ) -> CSSScopeRule {
        CSSScopeRule {
            grouping_rule: CSSGroupingRule::new_inherited(parent_stylesheet),
            scope_rule: RefCell::new(scope_rule),
        }
    }

    #[cfg_attr(crown, allow(crown::unrooted_must_root))]
    pub(crate) fn new(
        window: &Window,
        parent_stylesheet: &CSSStyleSheet,
        scope_rule: Arc<ScopeRule>,
        can_gc: CanGc,
    ) -> DomRoot<CSSScopeRule> {
        reflect_dom_object(
            Box::new(CSSScopeRule::new_inherited(parent_stylesheet, scope_rule)),
            window,
            can_gc,
        )
    }

    pub(crate) fn clone_rules(&self) -> Arc<Locked<CssRules>> {
        self.scope_rule.borrow().rules.clone()
    }
}

impl CSSScopeRuleMethods<crate::DomTypeHolder> for CSSScopeRule {
    /// <https://drafts.csswg.org/css-cascade-6/#dom-cssscoperule-start>
    fn GetStart(&self) -> Option<DOMString> {
        self.scope_rule
            .borrow()
            .bounds
            .start
            .as_ref()
            .and_then(|start| {
                let mut result = String::new();
                start.to_css(&mut CssWriter::new(&mut result)).ok()?;
                Some(result.into())
            })
    }

    /// <https://drafts.csswg.org/css-cascade-6/#dom-cssscoperule-end>
    fn GetEnd(&self) -> Option<DOMString> {
        self.scope_rule
            .borrow()
            .bounds
            .end
            .as_ref()
            .and_then(|end| {
                let mut result = String::new();
                end.to_css(&mut CssWriter::new(&mut result)).ok()?;
                Some(result.into())
            })
    }
}
