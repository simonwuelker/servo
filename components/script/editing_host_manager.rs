/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::dom::bindings::root::{Dom, MutNullableDom};
use crate::dom::document::Document;
use crate::dom::htmlelement::HTMLElement;

/// Keeps track of the active [editing host] in a [document](crate::dom::document::Document)
///
/// [editing host]: https://html.spec.whatwg.org/multipage/interaction.html#editing-host
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub struct EditingHostManager {
    focused_contenteditable_element: MutNullableDom<HTMLElement>,
    document: Dom<Document>,
}

impl EditingHostManager {
    pub(crate) fn new(document: &Document) -> Self {
        Self {
            focused_contenteditable_element: Default::default(),
            document: Dom::from_ref(document),
        }
    }

    pub(crate) fn set_focused_contenteditable_element(&self, element: Option<&HTMLElement>) {
        self.focused_contenteditable_element.set(element);
    }
}
