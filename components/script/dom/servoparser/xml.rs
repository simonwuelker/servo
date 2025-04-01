/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#![cfg_attr(crown, allow(crown::unrooted_must_root))]

use std::cell::Cell;

use markup5ever::buffer_queue::BufferQueue;
use markup5ever::{DecodingParser, ParserAction};
use script_bindings::trace::CustomTraceable;
use servo_url::ServoUrl;
use tendril::StrTendril;
use xml5ever::tokenizer::XmlTokenizer;
use xml5ever::tree_builder::XmlTreeBuilder;

use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{Dom, DomRoot};
use crate::dom::document::Document;
use crate::dom::htmlscriptelement::HTMLScriptElement;
use crate::dom::node::Node;
use crate::dom::servoparser::{ParsingAlgorithm, Sink};

#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub(crate) struct Tokenizer {
    #[ignore_malloc_size_of = "Defined in markup5ever"]
    inner: DecodingParser<XmlTokenizer<XmlTreeBuilder<Dom<Node>, Sink>>>,
}

impl Tokenizer {
    pub(crate) fn new(document: &Document, url: ServoUrl) -> Self {
        let sink = Sink {
            base_url: url,
            document: Dom::from_ref(document),
            current_line: Cell::new(1),
            script: Default::default(),
            parsing_algorithm: ParsingAlgorithm::Normal,
        };

        let tree_builder = XmlTreeBuilder::new(sink, Default::default());
        let tokenizer = XmlTokenizer::new(tree_builder, Default::default());

        Tokenizer {
            inner: DecodingParser::new(tokenizer, document.encoding()),
        }
    }

    pub(crate) fn feed_code_points(&self, chunk: StrTendril) {
        self.inner.input_stream().append(chunk);
    }

    pub(crate) fn feed_bytes(&self, chunk: &[u8]) {
        self.inner.input_stream().append_bytes(chunk);
    }

    pub(crate) fn end(&self) {
        self.inner.sink().end()
    }

    pub(crate) fn url(&self) -> &ServoUrl {
        &self.inner.sink().sink.sink.base_url
    }

    pub(crate) fn parse(&self) -> impl Iterator<Item = ParserAction<DomRoot<HTMLScriptElement>>> {
        self.inner.parse().flat_map(map_action)
    }

    pub(crate) fn finish_decoding_input(&self) {
        self.inner.input_stream().finish_decoding_input();
    }

    pub(crate) fn clear_input_stream(&self) {
        self.inner.input_stream().clear();
    }

    pub(crate) fn document_write<'a>(
        &'a self,
        input: &'a BufferQueue,
    ) -> impl Iterator<Item = ParserAction<DomRoot<HTMLScriptElement>>> + 'a {
        self.inner.document_write(input).flat_map(map_action)
    }

    pub(crate) fn push_script_input(&self, input: &BufferQueue) {
        self.inner.push_script_input(input);
    }

    pub(crate) fn notify_parser_blocking_script_loaded(&self) {
        self.inner.notify_parser_blocking_script_loaded();
    }
}

fn map_action(action: ParserAction<Dom<Node>>) -> Option<ParserAction<DomRoot<HTMLScriptElement>>> {
    let action = match action {
        ParserAction::StartOverWithEncoding(encoding) => {
            ParserAction::StartOverWithEncoding(encoding)
        },
        ParserAction::HandleScript(script) => {
            if let Some(script) = script.downcast::<HTMLScriptElement>() {
                ParserAction::HandleScript(DomRoot::from_ref(script))
            } else {
                return None;
            }
        },
    };
    Some(action)
}
