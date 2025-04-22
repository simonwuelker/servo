/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use dom_struct::dom_struct;
use js::rust::HandleObject;
use script_bindings::codegen::GenericUnionTypes::USVStringOrURLPatternInit;
use script_bindings::error::{Error, Fallible};
use script_bindings::reflector::Reflector;
use script_bindings::root::DomRoot;
use script_bindings::script_runtime::CanGc;
use script_bindings::str::USVString;

use crate::dom::bindings::codegen::Bindings::URLPatternBinding;
use crate::dom::bindings::codegen::Bindings::URLPatternBinding::URLPatternMethods;
use crate::dom::bindings::reflector::reflect_dom_object_with_proto;
use crate::dom::globalscope::GlobalScope;

/// <https://urlpattern.spec.whatwg.org/#urlpattern>
#[dom_struct]
pub(crate) struct URLPattern {
    reflector: Reflector,

    /// <https://urlpattern.spec.whatwg.org/#urlpattern-associated-url-pattern>
    #[no_trace]
    #[ignore_malloc_size_of = "defined in urlpattern crate"]
    associated_url_pattern: urlpattern::UrlPattern,
}

impl URLPattern {
    #[cfg_attr(crown, allow(crown::unrooted_must_root))]
    fn new_inherited(associated_url_pattern: urlpattern::UrlPattern) -> URLPattern {
        URLPattern {
            reflector: Reflector::new(),
            associated_url_pattern,
        }
    }

    /// <https://urlpattern.spec.whatwg.org/#urlpattern-initialize>
    pub(crate) fn initialize(
        global: &GlobalScope,
        proto: Option<HandleObject>,
        input: USVStringOrURLPatternInit,
        base_url: Option<USVString>,
        options: &URLPatternBinding::URLPatternOptions,
        can_gc: CanGc,
    ) -> Fallible<DomRoot<URLPattern>> {
        // The section below converts from servos types to the types used in the urlpattern crate
        let base_url = base_url.map(|usv_string| usv_string.0);
        let input = bindings_to_third_party::map_urlpattern_input(input, base_url.clone());
        let options = urlpattern::UrlPatternOptions {
            ignore_case: options.ignoreCase,
        };

        // Parse and initialize the URL pattern.
        let pattern_init =
            urlpattern::quirks::process_construct_pattern_input(input, base_url.as_deref())
                .map_err(|error| Error::Type(format!("{error}")))?;

        let pattern = urlpattern::UrlPattern::parse(pattern_init, options)
            .map_err(|error| Error::Type(format!("{error}")))?;

        let url_pattern = reflect_dom_object_with_proto(
            Box::new(URLPattern::new_inherited(pattern)),
            global,
            proto,
            can_gc,
        );
        Ok(url_pattern)
    }
}

impl URLPatternMethods<crate::DomTypeHolder> for URLPattern {
    // <https://urlpattern.spec.whatwg.org/#dom-urlpattern-urlpattern>
    fn Constructor(
        global: &GlobalScope,
        proto: Option<HandleObject>,
        can_gc: CanGc,
        input: USVStringOrURLPatternInit,
        base_url: USVString,
        options: &URLPatternBinding::URLPatternOptions,
    ) -> Fallible<DomRoot<URLPattern>> {
        URLPattern::initialize(global, proto, input, Some(base_url), options, can_gc)
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-urlpattern-input-options>
    fn Constructor_(
        global: &GlobalScope,
        proto: Option<HandleObject>,
        can_gc: CanGc,
        input: USVStringOrURLPatternInit,
        options: &URLPatternBinding::URLPatternOptions,
    ) -> Fallible<DomRoot<URLPattern>> {
        // Step 1. Run initialize given this, input, null, and options.
        URLPattern::initialize(global, proto, input, None, options, can_gc)
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-test>
    fn Test(
        &self,
        input: USVStringOrURLPatternInit,
        base_url: Option<USVString>,
    ) -> Fallible<bool> {
        // Step 1. Let result be the result of match given this’s associated URL pattern, input, and baseURL if given.
        let result = self.Exec(input, base_url)?;

        // Step 2. If result is null, return false.
        // Step 3. Return true.
        Ok(result.is_some())
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-exec>
    fn Exec(
        &self,
        input: USVStringOrURLPatternInit,
        base_url: Option<USVString>,
    ) -> Fallible<Option<URLPatternBinding::URLPatternResult>> {
        // Step 1. Return the result of match given this’s associated URL pattern, input, and baseURL if given.
        let base_url = base_url.map(|usv_string| usv_string.0);
        let match_input = urlpattern::quirks::process_match_input(
            bindings_to_third_party::map_urlpattern_input(input, base_url.clone()),
            base_url.as_deref(),
        )
        .map_err(|error| Error::Type(format!("{error}")))?;
        let Some((match_input, inputs)) = match_input else {
            return Ok(None);
        };

        let result = self
            .associated_url_pattern
            .exec(match_input)
            .map_err(|error| Error::Type(format!("{error}")))?;
        let Some(result) = result else {
            return Ok(None);
        };

        let result = URLPatternBinding::URLPatternResult {
            // TODO: Why does codegen want an Option here?
            protocol: Some(third_party_to_bindings::map_component_result(
                result.protocol,
            )),
            username: Some(third_party_to_bindings::map_component_result(
                result.username,
            )),
            password: Some(third_party_to_bindings::map_component_result(
                result.password,
            )),
            hostname: Some(third_party_to_bindings::map_component_result(
                result.hostname,
            )),
            port: Some(third_party_to_bindings::map_component_result(result.port)),
            pathname: Some(third_party_to_bindings::map_component_result(
                result.pathname,
            )),
            search: Some(third_party_to_bindings::map_component_result(result.search)),
            hash: Some(third_party_to_bindings::map_component_result(result.hash)),
            inputs: None,
        };

        Ok(Some(result))
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-protocol>
    fn Protocol(&self) -> USVString {
        // Step 1. Return this’s associated URL pattern’s protocol component’s pattern string.
        USVString(self.associated_url_pattern.protocol().to_owned())
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-username>
    fn Username(&self) -> USVString {
        // Step 1. Return this’s associated URL pattern’s username component’s pattern string.
        USVString(self.associated_url_pattern.username().to_owned())
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-password>
    fn Password(&self) -> USVString {
        // Step 1. Return this’s associated URL pattern’s password component’s pattern string.
        USVString(self.associated_url_pattern.password().to_owned())
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-hostname>
    fn Hostname(&self) -> USVString {
        // Step 1. Return this’s associated URL pattern’s hostname component’s pattern string.
        USVString(self.associated_url_pattern.hostname().to_owned())
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-port>
    fn Port(&self) -> USVString {
        // Step 1. Return this’s associated URL pattern’s port component’s pattern string.
        USVString(self.associated_url_pattern.port().to_owned())
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-pathname>
    fn Pathname(&self) -> USVString {
        // Step 1. Return this’s associated URL pattern’s pathname component’s pattern string.
        USVString(self.associated_url_pattern.pathname().to_owned())
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-search>
    fn Search(&self) -> USVString {
        // Step 1. Return this’s associated URL pattern’s search component’s pattern string.
        USVString(self.associated_url_pattern.search().to_owned())
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-hash>
    fn Hash(&self) -> USVString {
        // Step 1. Return this’s associated URL pattern’s hash component’s pattern string.
        USVString(self.associated_url_pattern.hash().to_owned())
    }

    /// <https://urlpattern.spec.whatwg.org/#dom-urlpattern-hasregexpgroups>
    fn HasRegExpGroups(&self) -> bool {
        // Step 1. If this’s associated URL pattern’s has regexp groups, then return true.
        // Step 2. Return false.
        self.associated_url_pattern.has_regexp_groups()
    }
}

mod bindings_to_third_party {
    use crate::dom::urlpattern::USVStringOrURLPatternInit;

    pub(super) fn map_urlpattern_input(
        input: USVStringOrURLPatternInit,
        base_url: Option<String>,
    ) -> urlpattern::quirks::StringOrInit {
        match input {
            USVStringOrURLPatternInit::USVString(usv_string) => {
                urlpattern::quirks::StringOrInit::String(usv_string.0)
            },
            USVStringOrURLPatternInit::URLPatternInit(pattern_init) => {
                let pattern_init = urlpattern::quirks::UrlPatternInit {
                    protocol: pattern_init
                        .protocol
                        .as_ref()
                        .map(|usv_string| usv_string.to_string()),
                    username: pattern_init
                        .username
                        .as_ref()
                        .map(|usv_string| usv_string.to_string()),
                    password: pattern_init
                        .password
                        .as_ref()
                        .map(|usv_string| usv_string.to_string()),
                    hostname: pattern_init
                        .hostname
                        .as_ref()
                        .map(|usv_string| usv_string.to_string()),
                    port: pattern_init
                        .port
                        .as_ref()
                        .map(|usv_string| usv_string.to_string()),
                    pathname: pattern_init
                        .pathname
                        .as_ref()
                        .map(|usv_string| usv_string.to_string()),
                    search: pattern_init
                        .search
                        .as_ref()
                        .map(|usv_string| usv_string.to_string()),
                    hash: pattern_init
                        .hash
                        .as_ref()
                        .map(|usv_string| usv_string.to_string()),
                    base_url,
                };
                urlpattern::quirks::StringOrInit::Init(pattern_init)
            },
        }
    }
}

mod third_party_to_bindings {
    use script_bindings::codegen::GenericBindings::URLPatternBinding;
    use script_bindings::codegen::GenericBindings::URLPatternBinding::URLPatternInit;
    use script_bindings::str::USVString;

    use crate::dom::urlpattern::USVStringOrURLPatternInit;

    pub(super) fn map_match_inputs(
        (match_input, base_url): urlpattern::quirks::Inputs,
    ) -> Vec<USVStringOrURLPatternInit> {
        let mut result = Vec::new();
        match match_input {
            urlpattern::quirks::StringOrInit::String(string) => {
                result.push(USVStringOrURLPatternInit::USVString(USVString(string)))
            },
            urlpattern::quirks::StringOrInit::Init(init) => {
                let init = URLPatternInit {
                    protocol: init.protocol.map(USVString),
                    username: init.username.map(USVString),
                    password: init.password.map(USVString),
                    hostname: init.hostname.map(USVString),
                    port: init.port.map(USVString),
                    pathname: init.pathname.map(USVString),
                    search: init.search.map(USVString),
                    hash: init.hash.map(USVString),
                    baseURL: init.base_url.map(USVString),
                };
                result.push(USVStringOrURLPatternInit::URLPatternInit(init))
            },
        }

        if let Some(base_url) = base_url {
            result.push(USVStringOrURLPatternInit::USVString(USVString(base_url)));
        }

        result
    }

    pub(super) fn map_component_result(
        component_result: urlpattern::UrlPatternComponentResult,
    ) -> URLPatternBinding::URLPatternComponentResult {
        URLPatternBinding::URLPatternComponentResult {
            // TODO: Why does codegen want an Option here?
            input: Some(USVString(component_result.input)),
            groups: None,
        }
    }
}
