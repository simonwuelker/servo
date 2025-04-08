/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use script_bindings::error::Fallible;
use script_bindings::str::USVString;

use crate::dom::bindings::codegen::Bindings::URLPatternBinding::URLPatternInit;
use crate::dom::urlpattern::tokenizer::{Token, TokenType, TokenizePolicy, tokenize};

/// <https://urlpattern.spec.whatwg.org/#constructor-string-parser>
struct ConstructorStringParser<'a> {
    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-input>
    input: &'a str,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-token-list>
    token_list: Vec<Token<'a>>,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-result>
    result: URLPatternInit,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-component-start>
    component_start: usize,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-token-index>
    token_index: usize,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-token-increment>
    token_increment: usize,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-group-depth>
    group_depth: usize,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-hostname-ipv6-bracket-depth>
    hostname_ipv6_bracket_depth: usize,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-protocol-matches-a-special-scheme-flag>
    protocol_matches_a_special_scheme: bool,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state>
    state: ParserState,
}

/// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParserState {
    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state-init>
    Init,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state-protocol>
    Protocol,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state-authority>
    Authority,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state-username>
    Username,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state-password>
    Password,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state-hostname>
    Hostname,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state-port>
    Port,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state-pathname>
    Pathname,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state-search>
    Search,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state-hash>
    Hash,

    /// <https://urlpattern.spec.whatwg.org/#constructor-string-parser-state-done>
    Done,
}

/// <https://urlpattern.spec.whatwg.org/#parse-a-constructor-string>
pub(super) fn parse_a_constructor_string(input: &str) -> Fallible<URLPatternInit> {
    // Step 1. Let parser be a new constructor string parser whose input is input and token list
    // is the result of running tokenize given input and "lenient".
    let token_list = tokenize(input, TokenizePolicy::Lenient)?;
    let mut parser = ConstructorStringParser::new(input, token_list);

    // Step 2. While parser’s token index is less than parser’s token list size:
    while parser.token_index < parser.token_list.len() {
        // Step 2.1 Set parser’s token increment to 1.
        parser.token_increment = 1;

        // Step 2.2 If parser’s token list[parser’s token index]'s type is "end" then:
        if parser.token_list[parser.token_index].token_type == TokenType::End {
            // Step 2.2.1 If parser’s state is "init":
            if parser.state == ParserState::Init {
                // Step 2.2.1.1 Run rewind given parser.
                parser.rewind();

                // Step 2.2.1.2 If the result of running is a hash prefix given parser is true,
                // then run change state given parser, "hash" and 1.
                if parser.is_a_hash_prefix() {
                    parser.change_state(ParserState::Hash, 1);
                }
                // Step 2.2.1.3 Otherwise if the result of running is a search prefix given parser is true:
                else if parser.is_a_search_prefix() {
                    // Step 2.2.1.3.1 Run change state given parser, "search" and 1.
                    parser.change_state(ParserState::Search, 1);
                }
                // Step 2.2.1.4 Otherwise:
                else {
                    // Step 2.2.1.4.1 Run change state given parser, "pathname" and 0.
                    parser.change_state(ParserState::Pathname, 1);
                }

                // Step 2.2.1.5 Increment parser’s token index by parser’s token increment.
                parser.token_index += parser.token_increment;

                // Step 2.2.1.6 Continue.
                continue;
            }

            // Step 2.2.2 If parser’s state is "authority":
            if parser.state == ParserState::Authority {
                // Step 2.2.2.1 Run rewind and set state given parser, and "hostname".
                parser.rewind_and_set_state(ParserState::Hostname);

                // Step 2.2.2.2 Increment parser’s token index by parser’s token increment.
                parser.token_index += parser.token_increment;

                // Step 2.2.2.3 Continue.
                continue;
            }

            // Step 2.2.3 Run change state given parser, "done" and 0.
            parser.change_state(ParserState::Done, 0);

            // Step 2.2.4 Break.
            break;
        }

        // Step 2.3 If the result of running is a group open given parser is true:
        if parser.is_a_group_open() {
            // Step 2.3.1 Increment parser’s group depth by 1.
            parser.group_depth += 1;

            // Step 2.3.2 Increment parser’s token index by parser’s token increment.
            parser.token_index += parser.token_increment;

            // Step 2.3.3 Continue.
            continue;
        }

        // Step 2.4 If parser’s group depth is greater than 0:
        if parser.group_depth > 0 {
            // Step 2.4.1 If the result of running is a group close given parser is true,
            // then decrement parser’s group depth by 1.
            if parser.is_a_group_close() {
                parser.group_depth -= 1;
            }
            // Step 2.4.2 Otherwise:
            else {
                // Step 2.4.2.1 Increment parser’s token index by parser’s token increment.
                parser.token_index += parser.token_increment;

                // Step 2.4.2.2 Continue.
                continue;
            }
        }

        // Step 2.5 Switch on parser’s state and run the associated steps:
        match parser.state {
            ParserState::Init => {
                // 1. If the result of running is a protocol suffix given parser is true:
                if parser.is_a_protocol_suffix() {
                    // Step 1.1 Run rewind and set state given parser and "protocol".
                    parser.rewind_and_set_state(ParserState::Protocol);
                }
            },
            ParserState::Protocol => {
                // Step 1. If the result of running is a protocol suffix given parser is true:
                if parser.is_a_protocol_suffix() {
                    // Step 1.1 Run compute protocol matches a special scheme flag given parser.
                    parser.compute_protocol_matches_a_special_scheme_flag()?;

                    // Step 1.2 Let next state be "pathname".
                    let mut next_state = ParserState::Pathname;

                    // Step 1.3 Let skip be 1.
                    let mut skip = 1;

                    // Step 1.4 If the result of running next is authority slashes given parser is true:
                    if parser.next_is_authority_slashes() {
                        // Step 1.4.1 Set next state to "authority".
                        next_state = ParserState::Authority;

                        // Step 1.4.2 Set skip to 3.
                        skip = 3;
                    }
                    // Step 1.5 Otherwise if parser’s protocol matches a special scheme flag is true,
                    // then set next state to "authority".
                    else if parser.protocol_matches_a_special_scheme {
                        next_state = ParserState::Authority;
                    }

                    // Step 1.6 Run change state given parser, next state, and skip.
                    parser.change_state(next_state, skip);
                }
            },
            ParserState::Authority => {
                // Step 1. If the result of running is an identity terminator given parser is true,
                // then run rewind and set state given parser and "username".
                if parser.is_an_identity_terminator() {
                    parser.rewind_and_set_state(ParserState::Username);
                }
                // Step 2. Otherwise if any of the following are true:
                // * the result of running is a pathname start given parser;
                // * the result of running is a search prefix given parser; or
                // * the result of running is a hash prefix given parser,
                // then run rewind and set state given parser and "hostname".
                else if parser.is_a_pathname_start() ||
                    parser.is_a_search_prefix() ||
                    parser.is_a_hash_prefix()
                {
                    parser.rewind_and_set_state(ParserState::Hostname);
                }
            },
            ParserState::Username => {
                // Step 1. If the result of running is a password prefix given parser is true,
                // then run change state given parser, "password", and 1.
                if parser.is_a_password_prefix() {
                    parser.change_state(ParserState::Password, 1);
                }
                // Step 2. Otherwise if the result of running is an identity terminator given parser is true,
                // then run change state given parser, "hostname", and 1.
                else if parser.is_an_identity_terminator() {
                    parser.change_state(ParserState::Hostname, 1);
                }
            },
            ParserState::Password => {
                // Step 1. If the result of running is an identity terminator given parser is true,
                // then run change state given parser, "hostname", and 1.
                if parser.is_an_identity_terminator() {
                    parser.change_state(ParserState::Hostname, 1);
                }
            },
            ParserState::Hostname => {
                // Step 1. If the result of running is an IPv6 open given parser is true,
                // then increment parser’s hostname IPv6 bracket depth by 1.
                if parser.is_an_ipv6_open() {
                    parser.hostname_ipv6_bracket_depth += 1;
                }
                // Step 2. Otherwise if the result of running is an IPv6 close given parser is true,
                // then decrement parser’s hostname IPv6 bracket depth by 1.
                else if parser.is_an_ipv6_close() {
                    parser.hostname_ipv6_bracket_depth -= 1;
                }
                // Step 3. Otherwise if the result of running is a port prefix given parser is true
                // and parser’s hostname IPv6 bracket depth is zero, then run change state given parser,
                // "port", and 1.
                else if parser.is_a_port_prefix() && parser.hostname_ipv6_bracket_depth == 0 {
                    parser.change_state(ParserState::Port, 1);
                }
                // Step 4. Otherwise if the result of running is a pathname start given parser is true,
                // then run change state given parser, "pathname", and 0.
                else if parser.is_a_pathname_start() {
                    parser.change_state(ParserState::Pathname, 0);
                }
                // Step 5. Otherwise if the result of running is a search prefix given parser is true,
                // then run change state given parser, "search", and 1.
                else if parser.is_a_search_prefix() {
                    parser.change_state(ParserState::Search, 1);
                }
                // Step 6. Otherwise if the result of running is a hash prefix given parser is true,
                // then run change state given parser, "hash", and 1.
                else if parser.is_a_hash_prefix() {
                    parser.change_state(ParserState::Hash, 1);
                }
            },
            ParserState::Port => {
                // Step 1. If the result of running is a pathname start given parser is true,
                // then run change state given parser, "pathname", and 0.
                if parser.is_a_pathname_start() {
                    parser.change_state(ParserState::Pathname, 0);
                }
                // Step 2. Otherwise if the result of running is a search prefix given parser is true,
                // then run change state given parser, "search", and 1.
                else if parser.is_a_search_prefix() {
                    parser.change_state(ParserState::Search, 1);
                }
                // Step 3. Otherwise if the result of running is a hash prefix given parser is true,
                // then run change state given parser, "hash", and 1.
                else if parser.is_a_hash_prefix() {
                    parser.change_state(ParserState::Hash, 1);
                }
            },
            ParserState::Pathname => {
                // Step 1. If the result of running is a search prefix given parser is true,
                // then run change state given parser, "search", and 1.
                if parser.is_a_search_prefix() {
                    parser.change_state(ParserState::Search, 1);
                }
                // Step 2. Otherwise if the result of running is a hash prefix given parser is true,
                // then run change state given parser, "hash", and 1.
                else if parser.is_a_hash_prefix() {
                    parser.change_state(ParserState::Hash, 1);
                }
            },
            ParserState::Search => {
                // Step 1. If the result of running is a hash prefix given parser is true,
                // then run change state given parser, "hash", and 1.
                if parser.is_a_hash_prefix() {
                    parser.change_state(ParserState::Hash, 1);
                }
            },
            ParserState::Hash => {
                // Step 1. Do nothing.
            },
            ParserState::Done => {
                // Step 1. Assert: This step is never reached.
                unreachable!()
            },
        }

        // Step 2.6 Increment parser’s token index by parser’s token increment.
        parser.token_index += parser.token_increment;
    }

    // Step 3. If parser’s result contains "hostname" and not "port",
    // then set parser’s result["port"] to the empty string.
    if parser.result.hostname.is_some() && parser.result.port.is_none() {
        parser.result.port = Some(Default::default());
    }

    // Step 4. Return parser’s result.
    Ok(parser.result)
}

impl<'a> ConstructorStringParser<'a> {
    fn new(input: &'a str, token_list: Vec<Token<'a>>) -> Self {
        Self {
            input,
            token_list,
            result: URLPatternInit::default(),
            component_start: 0,
            token_index: 0,
            token_increment: 1,
            group_depth: 0,
            hostname_ipv6_bracket_depth: 0,
            protocol_matches_a_special_scheme: false,
            state: ParserState::Init,
        }
    }

    /// <https://urlpattern.spec.whatwg.org/#change-state>
    fn change_state(&mut self, new_state: ParserState, skip: usize) {
        // Step 1. If parser’s state is not "init", not "authority", and not "done",
        // then set parser’s result[parser’s state] to the result of running make a component string given parser.
        if !matches!(
            self.state,
            ParserState::Init | ParserState::Authority | ParserState::Done
        ) {
            let component_string = self.make_a_component_string().to_owned();
            self.set_result_field(self.state, USVString(component_string))
        }

        // Step 2. If parser’s state is not "init" and new state is not "done", then:
        if self.state != ParserState::Init && new_state != ParserState::Done {
            // Step 2.1 If parser’s state is "protocol", "authority", "username", or "password";
            // new state is "port", "pathname", "search", or "hash";  and parser’s result["hostname"]
            // does not exist, then set parser’s result["hostname"] to the empty string.
            let parser_state_matches = matches!(
                self.state,
                ParserState::Protocol |
                    ParserState::Authority |
                    ParserState::Username |
                    ParserState::Password
            );
            let new_state_matches = matches!(
                new_state,
                ParserState::Port | ParserState::Pathname | ParserState::Search | ParserState::Hash
            );
            if parser_state_matches && new_state_matches && self.result.hostname.is_none() {
                self.result.hostname = Some(Default::default());
            }

            // Step 2.2 If parser’s state is "protocol", "authority", "username", "password", "hostname",
            // or "port"; new state is "search" or "hash"; and parser’s result["pathname"] does not exist, then:
            let parser_state_matches = matches!(
                self.state,
                ParserState::Protocol |
                    ParserState::Authority |
                    ParserState::Username |
                    ParserState::Password |
                    ParserState::Hostname |
                    ParserState::Port
            );
            let new_state_matches = matches!(new_state, ParserState::Search | ParserState::Hash);
            if parser_state_matches && new_state_matches && self.result.pathname.is_none() {
                // Step 2.2.1 If parser’s protocol matches a special scheme flag is true,
                // then set parser’s result["pathname"] to "/".
                if self.protocol_matches_a_special_scheme {
                    self.result.pathname = Some(USVString("/".into()));
                }
                // Step 2.2.2 Otherwise, set parser’s result["pathname"] to the empty string.
                else {
                    self.result.pathname = Some(Default::default());
                }
            }

            // Step 2.3 If parser’s state is "protocol", "authority", "username", "password", "hostname",
            // "port", or "pathname"; new state is "hash"; and parser’s result["search"] does not exist,
            // then set parser’s result["search"] to the empty string.
            let parser_state_matches = matches!(
                self.state,
                ParserState::Protocol |
                    ParserState::Authority |
                    ParserState::Username |
                    ParserState::Password |
                    ParserState::Hostname |
                    ParserState::Port |
                    ParserState::Pathname
            );
            if parser_state_matches &&
                new_state == ParserState::Hash &&
                self.result.search.is_none()
            {
                self.result.search = Some(Default::default());
            }
        }

        // Step 3. Set parser’s state to new state.
        self.state = new_state;

        // Step 4. Increment parser’s token index by skip.
        self.token_index += skip;

        // Step 5. Set parser’s component start to parser’s token index.
        self.component_start = self.token_index;

        // Step 6. Set parser’s token increment to 0.
        self.token_increment = 0;
    }

    /// <https://urlpattern.spec.whatwg.org/#rewind>
    fn rewind(&mut self) {
        // Step 1. Set parser’s token index to parser’s component start.
        self.token_index = self.component_start;

        // Step 2. Set parser’s token increment to 0.
        self.token_increment = 0;
    }

    /// <https://urlpattern.spec.whatwg.org/#rewind-and-set-state>
    fn rewind_and_set_state(&mut self, state: ParserState) {
        // Step 1. Run rewind given parser.
        self.rewind();

        // Step 2. Set parser’s state to state.
        self.state = state;
    }

    /// <https://urlpattern.spec.whatwg.org/#get-a-safe-token>
    fn get_a_safe_token(&self, index: usize) -> Token<'a> {
        // Step 1. If index is less than parser’s token list’s size, then return parser’s token list[index].
        if let Some(token) = self.token_list.get(index) {
            return *token;
        }

        // Step 2. Assert: parser’s token list’s size is greater than or equal to 1.
        debug_assert!(!self.token_list.is_empty());

        // Step 3. Let last index be parser’s token list’s size − 1.
        let last_index = self.token_list.len() - 1;

        // Step 4. Let token be parser’s token list[last index].
        let token = self.token_list[last_index];

        // Step 5. Assert: token’s type is "end".
        debug_assert_eq!(token.token_type, TokenType::End);

        // Step 6. Return token.
        token
    }

    /// <https://urlpattern.spec.whatwg.org/#is-a-non-special-pattern-char>
    fn is_a_non_special_pattern_char(&self, index: usize, value: &str) -> bool {
        // Step 1. Let token be the result of running get a safe token given parser and index.
        let token = self.get_a_safe_token(index);

        // Step 2. If token’s value is not value, then return false.
        if token.value != value {
            return false;
        }

        // Step 3. If any of the following are true:
        // * token’s type is "char";
        // * token’s type is "escaped-char"; or
        // * token’s type is "invalid-char",
        // then return true.
        if matches!(
            token.token_type,
            TokenType::Char | TokenType::EscapedChar | TokenType::InvalidChar
        ) {
            return true;
        }

        // Step 4. Return false.
        false
    }

    /// <https://urlpattern.spec.whatwg.org/#is-a-protocol-suffix>
    fn is_a_protocol_suffix(&self) -> bool {
        // Step 1. Return the result of running is a non-special pattern char given parser,
        // parser’s token index, and ":".
        self.is_a_non_special_pattern_char(self.token_index, ":")
    }

    /// <https://urlpattern.spec.whatwg.org/#next-is-authority-slashes>
    fn next_is_authority_slashes(&self) -> bool {
        // Step 1. If the result of running is a non-special pattern char given parser,
        // parser’s token index + 1, and "/" is false, then return false.
        if !self.is_a_non_special_pattern_char(self.token_index + 1, "/") {
            return false;
        }

        // Step 2. If the result of running is a non-special pattern char given parser,
        // parser’s token index + 2, and "/" is false, then return false.
        if !self.is_a_non_special_pattern_char(self.token_index + 2, "/") {
            return false;
        }

        // Step 3. Return true.
        true
    }

    /// <https://urlpattern.spec.whatwg.org/#is-an-identity-terminator>
    fn is_an_identity_terminator(&self) -> bool {
        // Step 1. Return the result of running is a non-special pattern char given parser,
        // parser’s token index, and "@".
        self.is_a_non_special_pattern_char(self.token_index, "@")
    }

    /// <https://urlpattern.spec.whatwg.org/#is-a-password-prefix>
    fn is_a_password_prefix(&self) -> bool {
        // Step 1. Return the result of running is a non-special pattern char given parser,
        // parser’s token index, and ":".
        self.is_a_non_special_pattern_char(self.token_index, ":")
    }

    /// <https://urlpattern.spec.whatwg.org/#is-a-port-prefix>
    fn is_a_port_prefix(&self) -> bool {
        // Step 1. Return the result of running is a non-special pattern char given parser,
        // parser’s token index, and ":".
        self.is_a_non_special_pattern_char(self.token_index, ":")
    }

    /// <https://urlpattern.spec.whatwg.org/#is-a-pathname-start>
    fn is_a_pathname_start(&self) -> bool {
        // Step 1. Return the result of running is a non-special pattern char given parser,
        // parser’s token index, and "/".
        self.is_a_non_special_pattern_char(self.token_index, "/")
    }

    /// <https://urlpattern.spec.whatwg.org/#is-a-search-prefix>
    fn is_a_search_prefix(&self) -> bool {
        // Step 1. If result of running is a non-special pattern char given parser,
        // parser’s token index and "?" is true, then return true.
        if self.is_a_non_special_pattern_char(self.token_index, "?") {
            return true;
        }

        // Step 2. If parser’s token list[parser’s token index]'s value is not "?", then return false.
        if self.token_list[self.token_index].value != "?" {
            return false;
        }

        // Step 3. Let previous index be parser’s token index − 1.
        // Step 4. If previous index is less than 0, then return true.
        if self.token_index == 0 {
            return true;
        }
        let previous_index = self.token_index - 1;

        // Step 5. Let previous token be the result of running get a safe token given parser and previous index.
        let previous_token = self.get_a_safe_token(previous_index);

        // Step 6. If any of the following are true, then return false:
        // * previous token’s type is "name".
        // * previous token’s type is "regexp".
        // * previous token’s type is "close".
        // * previous token’s type is "asterisk".
        if matches!(
            previous_token.token_type,
            TokenType::Name | TokenType::Regexp | TokenType::Close | TokenType::Asterisk
        ) {
            return false;
        }

        // Step 7. Return true.
        true
    }

    /// <https://urlpattern.spec.whatwg.org/#is-a-hash-prefix>
    fn is_a_hash_prefix(&self) -> bool {
        // Step 1. Return the result of running is a non-special pattern char given parser,
        // parser’s token index and "#".
        self.is_a_non_special_pattern_char(self.token_index, "#")
    }

    /// <https://urlpattern.spec.whatwg.org/#is-a-group-open>
    fn is_a_group_open(&self) -> bool {
        // Step 1. If parser’s token list[parser’s token index]'s type is "open", then return true.
        // Step 2. Otherwise return false.
        self.token_list[self.token_index].token_type == TokenType::Open
    }

    /// <https://urlpattern.spec.whatwg.org/#is-a-group-close>
    fn is_a_group_close(&self) -> bool {
        // Step 1. If parser’s token list[parser’s token index]'s type is "close", then return true.
        // Step 2. Otherwise return false.
        self.token_list[self.token_index].token_type == TokenType::Close
    }

    /// <https://urlpattern.spec.whatwg.org/#is-an-ipv6-open>
    fn is_an_ipv6_open(&self) -> bool {
        // Step 1. Return the result of running is a non-special pattern char given parser,
        // parser’s token index, and "[".
        self.is_a_non_special_pattern_char(self.token_index, "[")
    }

    /// <https://urlpattern.spec.whatwg.org/#is-an-ipv6-close>
    fn is_an_ipv6_close(&self) -> bool {
        // Step 1. Return the result of running is a non-special pattern char given parser,
        // parser’s token index, and "]".
        self.is_a_non_special_pattern_char(self.token_index, "]")
    }

    /// <https://urlpattern.spec.whatwg.org/#make-a-component-string>
    fn make_a_component_string(&self) -> &'a str {
        // Step 1. Assert: parser’s token index is less than parser’s token list’s size.
        // Step 2. Let token be parser’s token list[parser’s token index].
        let token = self.token_list.get(self.token_index).unwrap();

        // Step 3. Let component start token be the result of running
        // get a safe token given parser and parser’s component start.
        let component_start_token = self.get_a_safe_token(self.component_start);

        // Step 4. Let component start input index be component start token’s index.
        let component_start_input_index = component_start_token.index;

        // Step 4. Let end index be token’s index.
        let end_index = token.index;

        // Step 5. Return the code point substring from component start input index
        // to end index within parser’s input.
        &self.input[component_start_input_index..end_index]
    }

    /// <https://urlpattern.spec.whatwg.org/#compute-protocol-matches-a-special-scheme-flag>
    fn compute_protocol_matches_a_special_scheme_flag(&mut self) -> Fallible<()> {
        // FIXME: The way we currently construct components does not allow us to implement this algorithm.
        Ok(())
    }

    fn set_result_field(&mut self, field: ParserState, value: USVString) {
        match field {
            ParserState::Protocol => self.result.protocol = Some(value),
            ParserState::Username => self.result.username = Some(value),
            ParserState::Password => self.result.password = Some(value),
            ParserState::Hostname => self.result.hostname = Some(value),
            ParserState::Port => self.result.port = Some(value),
            ParserState::Pathname => self.result.pathname = Some(value),
            ParserState::Search => self.result.search = Some(value),
            ParserState::Hash => self.result.hash = Some(value),
            ParserState::Authority | ParserState::Init | ParserState::Done => unreachable!(),
        }
    }
}
