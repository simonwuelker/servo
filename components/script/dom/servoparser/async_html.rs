/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use script_bindings::script_runtime::CanGc;
use crate::dom::servoparser::ServoQuirksMode;
use html5ever::local_name;
use html5ever::ns;
use crate::dom::bindings::codegen::Bindings::HTMLTemplateElementBinding::HTMLTemplateElementMethods;
use crate::dom::bindings::codegen::Bindings::NodeBinding::NodeMethods;
use crate::dom::bindings::inheritance::Castable;
use crate::dom::bindings::root::{Dom, DomRoot, MutNullableDom};
use crate::dom::bindings::str::DOMString;
use crate::dom::bindings::trace::JSTraceable;
use crate::dom::comment::Comment;
use crate::dom::document::Document;
use crate::dom::documenttype::DocumentType;
use crate::dom::element::{Element, ElementCreator};
use crate::dom::htmlformelement::{FormControlElementHelpers, HTMLFormElement};
use crate::dom::htmlscriptelement::HTMLScriptElement;
use crate::dom::htmltemplateelement::HTMLTemplateElement;
use crate::dom::node::Node;
use crate::dom::processinginstruction::ProcessingInstruction;
use crate::dom::servoparser::{ElementAttribute, create_element_for_token, ParsingAlgorithm};
use crate::dom::virtualmethods::vtable_for;
use html5ever::{Attribute as HtmlAttribute, ExpandedName, QualName};
use html5ever::buffer_queue::BufferQueue;
use html5ever::tendril::{SendTendril, StrTendril, Tendril};
use html5ever::tendril::fmt::UTF8;
use markup5ever::TokenizerResult;
use html5ever::tokenizer::{Tokenizer as HtmlTokenizer, TokenizerOpts};
use html5ever::tree_builder::{ElementFlags, NodeOrText as HtmlNodeOrText, NextParserState, QuirksMode};
use html5ever::tree_builder::{TreeSink, TreeBuilder, TreeBuilderOpts};
use js::jsapi::JSTracer;
use servo_url::ServoUrl;
use std::borrow::Cow;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::collections::vec_deque::VecDeque;
use std::mem;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// Unique identifier for nodes
type ParseNodeId = usize;

#[derive(Clone, Default, JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
struct ParseOperationExecutor {
    nodes: HashMap<ParseNodeId, Dom<Node>>,
}

impl ParseOperationExecutor {
    fn new(document: Option<&Document>) -> ParseOperationExecutor {
        let mut executor = ParseOperationExecutor::default();

        if let Some(document) = document {
            executor.insert_node(0, Dom::from_ref(doc.upcast()));
        }
        executor
    }

    /// Insert a new node with the given id
    ///
    /// ### Panics
    /// Panics if there is already a node with the id.
    fn insert_node(&self, id: ParseNodeId, node: DomRoot<Node>) {
        assert!(self.nodes.borrow_mut().insert(id, node).is_none());
    }

    fn get_node<'a>(&'a self, id: &ParseNodeId) -> Ref<'a, Node> {
        Ref::map(self.nodes.borrow(), |nodes| {
            nodes.get(id).expect("Node not found!")
        })
    }

    fn append_before_sibling(&self, sibling: ParseNodeId, node_or_text: NodeOrText, can_gc: CanGc) {
        let node = match node_or_text {
            NodeOrText::Node(n) => {
                HtmlNodeOrText::AppendNode(DomRoot::from_ref(&**self.get_node(&n.id)))
            },
            NodeOrText::Text(text) => HtmlNodeOrText::AppendText(Tendril::from(text)),
        };
        let sibling = &**self.get_node(&sibling);
        let parent = &*sibling
            .GetParentNode()
            .expect("append_before_sibling called on node without parent");

        super::insert(parent, Some(sibling), node, self.parsing_algorithm, can_gc);
    }

    fn append(&self, parent: ParseNodeId, node: NodeOrText, can_gc: CanGc) {
        let node = match node {
            NodeOrText::Node(n) => {
                HtmlNodeOrText::AppendNode(Dom::from_ref(&**self.get_node(&n.id)))
            },
            NodeOrText::Text(text) => HtmlNodeOrText::AppendText(Tendril::from(text)),
        };

        let parent = &**self.get_node(&parent);
        super::insert(parent, None, node, self.parsing_algorithm, can_gc);
    }

    fn has_parent_node(&self, node: ParseNodeId) -> bool {
        self.get_node(&node).has_parent()
    }

    /// Return true iff the two nodes are in the same tree
    fn same_tree(&self, x: ParseNodeId, y: ParseNodeId) -> bool {
        let x = self.get_node(&x);
        let y = self.get_node(&y);

        let x = x.downcast::<Element>().expect("Element node expected");
        let y = y.downcast::<Element>().expect("Element node expected");
        x.is_in_same_home_subtree(y)
    }

    fn process_operation(&self, op: ParseOperation, can_gc: CanGc) {
        let document = DomRoot::from_ref(&**self.get_node(&0));
        let document = document
            .downcast::<Document>()
            .expect("Root node should be a document");
        match op {
            ParseOperation::GetTemplateContents { target, contents } => {
                let target = DomRoot::from_ref(&**self.get_node(&target));
                let template = target
                    .downcast::<HTMLTemplateElement>()
                    .expect("Tried to extract contents from non-template element while parsing");
                self.insert_node(contents, DomRoot::upcast(template.Content(can_gc)));
            },
            ParseOperation::CreateElement {
                node,
                name,
                attrs,
                current_line,
            } => {
                let attrs = attrs
                    .into_iter()
                    .map(|attr| ElementAttribute::new(attr.name, DOMString::from(attr.value)))
                    .collect();
                let element = create_element_for_token(
                    name,
                    attrs,
                    &self.document,
                    ElementCreator::ParserCreated(current_line),
                    ParsingAlgorithm::Normal,
                    can_gc,
                );
                self.insert_node(node, Dom::from_ref(element.upcast()));
            },
            ParseOperation::CreateComment { text, node } => {
                let comment = Comment::new(DOMString::from(text), document, None, can_gc);
                self.insert_node(node, DomRoot::upcast(comment));
            },
            ParseOperation::AppendBeforeSibling { sibling, node } => {
                self.append_before_sibling(sibling, node, can_gc);
            },
            ParseOperation::Append { parent, node } => {
                self.append(parent, node, can_gc);
            },
            ParseOperation::AppendBasedOnParentNode {
                element,
                prev_element,
                node,
            } => {
                if self.has_parent_node(element) {
                    self.append_before_sibling(element, node, can_gc);
                } else {
                    self.append(prev_element, node, can_gc);
                }
            },
            ParseOperation::AppendDoctypeToDocument {
                name,
                public_id,
                system_id,
            } => {
                let doctype = DocumentType::new(
                    DOMString::from(name),
                    Some(DOMString::from(public_id)),
                    Some(DOMString::from(system_id)),
                    document,
                    can_gc,
                );

                document
                    .upcast::<Node>()
                    .AppendChild(doctype.upcast(), can_gc)
                    .expect("Appending failed");
            },
            ParseOperation::AddAttrsIfMissing { target, attrs } => {
                let node = self.get_node(&target);
                let elem = node
                    .downcast::<Element>()
                    .expect("tried to set attrs on non-Element in HTML parsing");
                for attr in attrs {
                    elem.set_attribute_from_parser(
                        attr.name,
                        DOMString::from(attr.value),
                        None,
                        can_gc,
                    );
                }
            },
            ParseOperation::RemoveFromParent { target } => {
                if let Some(ref parent) = self.get_node(&target).GetParentNode() {
                    parent.RemoveChild(&self.get_node(&target), can_gc).unwrap();
                }
            },
            ParseOperation::MarkScriptAlreadyStarted { node } => {
                self.get_node(&node).downcast::<HTMLScriptElement>().unwrap().set_already_started(true);
            },
            ParseOperation::ReparentChildren { parent, new_parent } => {
                let parent = self.get_node(&parent);
                let new_parent = self.get_node(&new_parent);
                while let Some(child) = parent.GetFirstChild() {
                    new_parent.AppendChild(&child, can_gc).unwrap();
                }
            },
            ParseOperation::AssociateWithForm {
                target,
                form,
                element,
                prev_element,
            } => {
                let tree_node = prev_element.map_or(element, |prev| {
                    if self.has_parent_node(element) {
                        element
                    } else {
                        prev
                    }
                });

                if !self.same_tree(tree_node, form) {
                    return;
                }
                let form = self.get_node(&form);
                let form = DomRoot::downcast::<HTMLFormElement>(DomRoot::from_ref(&**form))
                    .expect("Owner must be a form element");

                let node = self.get_node(&target);
                let elem = node.downcast::<Element>();
                let control = elem.and_then(|e| e.as_maybe_form_control());

                if let Some(control) = control {
                    control.set_form_owner_from_parser(&form, can_gc);
                } else {
                    // TODO remove this code when keygen is implemented.
                    assert!(node.NodeName() == "KEYGEN", "Unknown form-associatable element");
                }
            },
            ParseOperation::Pop { node } => {
                vtable_for(&self.get_node(&node)).pop();
            },
            ParseOperation::CreatePI { node, target, data } => {
                let pi = ProcessingInstruction::new(
                    DOMString::from(target),
                    DOMString::from(data),
                    document,
                    can_gc,
                );
                self.insert_node(node, Dom::from_ref(pi.upcast()));
            },
            ParseOperation::SetQuirksMode { mode } => {
                document.set_quirks_mode(mode);
            },
        }
    }
}

#[derive(Clone, JSTraceable, MallocSizeOf)]
pub(crate) struct ParseNode {
    /// A unique handle for this node used by the [ParseOperationExecutor] to identify it.
    id: ParseNodeId,
    #[no_trace]
    qual_name: Option<QualName>,
}

#[derive(JSTraceable, MallocSizeOf)]
enum NodeOrText {
    Node(ParseNode),
    Text(String),
}

#[derive(JSTraceable, MallocSizeOf)]
struct Attribute {
    #[no_trace]
    name: QualName,
    value: String,
}

#[derive(JSTraceable, MallocSizeOf)]
enum ParseOperation {
    GetTemplateContents {
        target: ParseNodeId,
        contents: ParseNodeId,
    },
    CreateElement {
        node: ParseNodeId,
        #[no_trace]
        name: QualName,
        attrs: Vec<Attribute>,
        current_line: u64,
    },
    CreateComment {
        text: String,
        node: ParseNodeId,
    },
    AppendBeforeSibling {
        sibling: ParseNodeId,
        node: NodeOrText,
    },
    AppendBasedOnParentNode {
        element: ParseNodeId,
        prev_element: ParseNodeId,
        node: NodeOrText,
    },
    Append {
        parent: ParseNodeId,
        node: NodeOrText,
    },
    AppendDoctypeToDocument {
        name: String,
        public_id: String,
        system_id: String,
    },
    AddAttrsIfMissing {
        target: ParseNodeId,
        attrs: Vec<Attribute>,
    },
    RemoveFromParent {
        target: ParseNodeId,
    },
    MarkScriptAlreadyStarted {
        node: ParseNodeId,
    },
    ReparentChildren {
        parent: ParseNodeId,
        new_parent: ParseNodeId,
    },
    AssociateWithForm {
        target: ParseNodeId,
        form: ParseNodeId,
        element: ParseNodeId,
        prev_element: Option<ParseNodeId>,
    },

    CreatePI {
        node: ParseNodeId,
        target: String,
        data: String,
    },
    Pop {
        node: ParseNodeId,
    },
    SetQuirksMode {
        #[ignore_malloc_size_of = "Defined in style"]
        #[no_trace]
        mode: ServoQuirksMode,
    },
}

fn create_buffer_queue(mut buffers: VecDeque<SendTendril<UTF8>>) -> BufferQueue {
    let buffer_queue = BufferQueue::default();
    while let Some(st) = buffers.pop_front() {
        buffer_queue.push_back(StrTendril::from(st));
    }
    buffer_queue
}

/// Messages from the parser thread to the main thread.
#[derive(MallocSizeOf)]
enum ParserThreadToMainThreadMessage {
    TokenizerResultDone {
        #[ignore_malloc_size_of = "Defined in html5ever"]
        updated_input: VecDeque<SendTendril<UTF8>>,
        speculative_parsing_mode: bool,
    },

    TokenizerResultScript {
        script: ParseNode,
        #[ignore_malloc_size_of = "Defined in html5ever"]
        updated_input: VecDeque<SendTendril<UTF8>>,
        speculative_parsing_mode: bool,
    },

    /// Sent to Tokenizer to signify HtmlTokenizer's end method has returned
    End,

    /// The tokenizer on the main thread receives the h5e's tokenizer's state, which will
    /// be used to reconstruct the original tokenizer to parse document.write()'s contents.
    HtmlTokenizerInternalState(
        #[ignore_malloc_size_of = "Defined in html5ever"]
        SendableTokenizer<SendableTreeBuilder<ParseNode, SendableSink>>,
    ),

    // From Sink
    ProcessOperation(ParseOperation),
    SpeculativeParseOps(VecDeque<ParseOperation>),
}

/// Message from the main thread to the parser thread
#[derive(MallocSizeOf)]
enum MainThreadToParserThreadMessage {
    /// Causes the parser thread to immediately respond with `ParserThreadToMainThreadMessage::HtmlTokenizerInternalState` containing
    /// the state before speculative execution started.
    Feed {
        #[ignore_malloc_size_of = "Defined in html5ever"]
        input: VecDeque<SendTendril<UTF8>>,
        should_parse_speculatively: bool
    },
    /// html5ever is done parsing the input
    End,
    SetPlainTextState,
    /// `document.write` was called and we need to undo the progress made during speculative
    /// parsing.
    RestoreInternalState {
        #[ignore_malloc_size_of = "Defined in html5ever"]
        tok_internal_state: SendableTokenizer<SendableTreeBuilder<ParseNode, SendableSink>>
    },
    /// Send all the speculatively parsed tree operations to the main thread.
    ///
    /// The parser thread should immediately respond with `ParserThreadToMainThreadMessage::SpeculativeParseOps`,
    /// containing all the speculatively parsed operations.
    FlushTreeOps,
}

#[derive(JSTraceable, MallocSizeOf)]
pub enum TokenizerState {
    /// Default state, executes parse ops sent to it by the Sink
    ExecutingParseOps,
    /// HtmlTokenizer needed to parse content from calls to document.write() (if any) synchronously
    SpeculativeParsing {
        #[ignore_malloc_size_of = "Defined in html5ever"]
        tokenizer: HtmlTokenizer<TreeBuilder<ParseNode, Sink>>,
        document_write_called: bool,
        /// Is the topmost-level script a pending-parsing-blocking script?
        waiting_on_script: bool,
    },
}

/// The async HTML Tokenizer consists of two separate types working together: the Tokenizer
/// (defined below), which lives on the main thread, and the HtmlTokenizer, defined in html5ever, which
/// lives on the parser thread.
/// Steps:
/// 1. A call to Tokenizer::new will spin up a new parser thread, creating an HtmlTokenizer instance,
///    which starts listening for messages from Tokenizer.
/// 2. Upon receiving an input from ServoParser, the Tokenizer forwards it to HtmlTokenizer, where it starts
///    creating the necessary tree actions based on the input.
/// 3. HtmlTokenizer sends these tree actions to the Tokenizer as soon as it creates them. The Tokenizer
///    then executes the received actions.
///
/// ```text
///    _____________                           _______________
///   |             |                         |               |
///   |             |                         |               |
///   |             |   MainThreadToParserThreadMessage    |               |
///   |             |------------------------>| HtmlTokenizer |
///   |             |                         |               |
///   |  Tokenizer  |     ParserThreadToMainThreadMessage      |               |
///   |             |<------------------------|    ________   |
///   |             |                         |   |        |  |
///   |             |     ParserThreadToMainThreadMessage      |   |  Sink  |  |
///   |             |<------------------------|---|        |  |
///   |             |                         |   |________|  |
///   |_____________|                         |_______________|
/// ```
///
/// When parsing input speculatively, the parse operations are buffered in the parser thread until
/// `MainThreadToParserThreadMessage::FlushTreeOps` is sent, at which point it transitions back to sending
/// individual operations back to the main thread.
#[derive(JSTraceable, MallocSizeOf)]
#[cfg_attr(crown, crown::unrooted_must_root_lint::must_root)]
pub struct Tokenizer {
    receiver: Receiver<ParserThreadToMainThreadMessage>,
    html_tokenizer_sender: Sender<MainThreadToParserThreadMessage>,
    url: ServoUrl,
    state: TokenizerState,

    /// Used to build the DOM using the instructions received from the parser thread.
    executor: ParseOperationExecutor,

    /// Result of having speculatively parsed some input.
    pending_result: Option<MutNullableDom<HTMLScriptElement>>,
}

impl Tokenizer {
    pub fn new(
            document: &Document,
            url: ServoUrl,
            fragment_context: Option<super::FragmentContext>)
            -> Self {
        // Messages from the Tokenizer (main thread) to HtmlTokenizer (parser thread)
        let (to_html_tokenizer_sender, html_tokenizer_receiver) = channel();
        // Messages from HtmlTokenizer and Sink (both in parser thread) to Tokenizer (main thread)
        let (to_tokenizer_sender, tokenizer_receiver) = channel();

        let mut tokenizer = Tokenizer {
            receiver: tokenizer_receiver,
            html_tokenizer_sender: to_html_tokenizer_sender,
            url,
            state: TokenizerState::ExecutingParseOps,
            executor: ParseOperationExecutor::new(Some(document)),
            pending_result: None,
        };

        let mut sink = Sink::new(to_tokenizer_sender.clone());
        let mut ctxt_parse_node = None;
        let mut form_parse_node = None;
        let mut fragment_context_is_some = false;
        if let Some(fc) = fragment_context {
            let node = sink.new_parse_node();
            tokenizer.executor.insert_node(node.id, Dom::from_ref(fc.context_elem));
            ctxt_parse_node = Some(node);

            form_parse_node = fc.form_elem.map(|form_elem| {
                let node = sink.new_parse_node();
                tokenizer.executor.insert_node(node.id, Dom::from_ref(form_elem));
                node
            });
            fragment_context_is_some = true;
        };
        let sendable_sink = sink.get_sendable();

        // Create new thread for HtmlTokenizer. This is where parser actions
        // will be generated from the input provided. These parser actions are then passed
        // onto the main thread to be executed.
        thread::Builder::new().name(String::from("html-parser")).spawn(move || {
            run(sendable_sink,
                fragment_context_is_some,
                ctxt_parse_node,
                form_parse_node,
                to_tokenizer_sender,
                html_tokenizer_receiver);
        }).expect("HTML Parser thread spawning failed");

        tokenizer
    }

    pub fn feed(&mut self, input: &mut BufferQueue) -> Result<(), DomRoot<HTMLScriptElement>> {
        match self.state {
            TokenizerState::SpeculativeParsing {
                ref mut tokenizer,
                ref mut document_write_called,
                ..
            } => {
                // Only document.write() calls can call tokenizer's feed function when
                // it is in speculative parsing state.
                *document_write_called = true;

                // Previous speculative parsing result has been invalidated now, so discard it.
                self.pending_result.take();

                match tokenizer.feed(input) {
                    TokenizerResult::Done => Ok(()),
                    TokenizerResult::Script(script) => {
                        let SinkState::ParsingDocWriteContents(executor) = &tokenizer.sink.sink.state else {
                            unreachable!();
                        };
                        let script_node = executor.get_node(&script.id);
                        Err(DomRoot::from_ref(script_node.downcast().unwrap()))
                    },
                }
            },
            TokenizerState::ExecutingParseOps => {
                // If a pending result is present (result generated during speculative parsing, while a
                // pending-parsing-blocking script was being prepared/executed), then we take the result and
                // return it instead of feeding.
                if let Some(result) = self.pending_result.take() {
                        return match result.get() {
                            Some(script) => Err(script),
                            None => Ok(()),
                        };
                }

                // Make input shareable to send it to the parser thread.
                let mut send_tendrils: VecDeque<SendTendril> = input.into_iter().map(SendTendril::from).collect();

                // Send message to parser thread, asking it to start reading from the input.
                // Parser operation messages will be sent to main thread as they are evaluated.
                self.html_tokenizer_sender.send(
                    MainThreadToParserThreadMessage::Feed {
                        input: send_tendrils,
                        should_parse_speculatively: false
                    }).unwrap();

                // Execute the parse operations we receive from the parser thread until it gets
                // stuck on a <script>/EOF.
                loop {
                    match self.receiver.recv().expect("Unexpected channel panic in main thread.") {
                        ParserThreadToMainThreadMessage::ProcessOperation(operation) => self.executor.process_operation(operation),
                        ParserThreadToMainThreadMessage::TokenizerResultDone { updated_input, speculative_parsing_mode } => {
                            assert_eq!(speculative_parsing_mode, false, "parser thread parsed speculatively, but we told it not to");

                            let buffer_queue = create_buffer_queue(updated_input);
                            *input = buffer_queue;
                            return Ok(());
                        },
                        ParserThreadToMainThreadMessage::TokenizerResultScript { script, updated_input, speculative_parsing_mode } => {
                            assert_eq!(speculative_parsing_mode, false, "parser thread parsed speculatively, but we told it not to");

                            let buffer_queue = create_buffer_queue(updated_input);
                            *input = buffer_queue;
                            let script = self.executor.get_node(&script.id);
                            return Err(DomRoot::from_ref(script.downcast().unwrap()));
                        },
                        _ => unreachable!("parser thread sent unexpected message"),
                    };
                }
            }
        }
    }

    pub fn end(&mut self) {
        self.html_tokenizer_sender.send(MainThreadToParserThreadMessage::End).unwrap();

        // Execute the remaining parse operations until the parser is done too
        loop {
            match self.receiver.recv().expect("Unexpected channel panic in main thread.") {
                ParserThreadToMainThreadMessage::ProcessOperation(parse_op) => self.executor.process_operation(parse_op),
                ParserThreadToMainThreadMessage::End => return,
                _ => unreachable!(),
            };
        }
    }

    pub fn url(&self) -> &ServoUrl {
        &self.url
    }

    pub fn set_plaintext_state(&mut self) {
        self.html_tokenizer_sender.send(MainThreadToParserThreadMessage::SetPlainTextState).unwrap();
    }

    pub fn start_speculative_parsing(&mut self, input: &mut BufferQueue) {
        input.notify_speculative_parsing_has_started();


        let send_tendrils: VecDeque<SendTendril> = input.clone().into_iter().map(SendTendril::from).collect();

        // Send message to parser thread, asking it to start reading from the input.
        // Parser operation messages will be enqueued. Under the right conditions,
        // these parser operations will be sent to the main thread to be executed.
        self.html_tokenizer_sender.send(
            MainThreadToParserThreadMessage::Feed {
                input: send_tendrils,
                should_parse_speculatively: true
            }).unwrap();


        // Receive the tokenizer from the parser thread (???)
        match self.receiver.recv().expect("Unexpected channel panic in main thread.") {
            ParserThreadToMainThreadMessage::HtmlTokenizerInternalState(sendable_tok) => {
                let mut tokenizer: HtmlTokenizer<TreeBuilder<ParseNode, Sink>> = HtmlTokenizer::get_self_from_sendable(
                                                                                     sendable_tok
                                                                                 );
                // transfer the executor to the Sink, while leaving a dummy executor in place.
                tokenizer.sink.sink.state = SinkState::ParsingDocWriteContents(
                    mem::replace(&mut self.executor, ParseOperationExecutor::new(None))
                );
                self.state = TokenizerState::SpeculativeParsing {
                    tokenizer: tokenizer,
                    document_write_called: false,
                    waiting_on_script: false,
                };
            },
            _ => unreachable!(),
        };
    }

    pub fn end_speculative_parsing(&mut self,
                                   input: &mut BufferQueue,
                                   document_has_pending_parsing_blocking_script: bool) {
        match self.state {
            TokenizerState::SpeculativeParsing {
                waiting_on_script,
                ..
            } => {
                *waiting_on_script = document_has_pending_parsing_blocking_script;
                if *document_has_pending_parsing_blocking_script {
                    return;
                }
            },
            TokenizerState::ExecutingParseOps => unreachable!("cannot stop speculative parser that never started"),
        };

        let old_state = mem::replace(&mut self.state, TokenizerState::ExecutingParseOps);
        match old_state {
            TokenizerState::SpeculativeParsing {
                mut tokenizer,
                document_write_called,
                waiting_on_script,
            } => {

                assert_eq!(waiting_on_script, false, "should not execute parse ops while waiting on script");

                // Block until the parser thread reaches a point where it cannot continue - this
                // can either be a <script> tag or the end of the input.
                let msg = self.receiver.recv().expect("Unexpected channel panic in main thread.");
                match tokenizer.sink.sink.state {
                    SinkState::ParsingDocWriteContents(ref mut executor) => {
                        // self.executor contains the dummy executor we had assigned to it in
                        // `starts_speculative_parsing`; this line ensures that the tokenizer once again
                        // repossesses it.
                        mem::swap(&mut self.executor, executor);
                    }
                    _ => unreachable!(),
                };

                if document_write_called {
                    // This is the "bad case" for the speculative parser: The script called
                    // document.write, and we have to throw all our progress away to start over.
                    tokenizer.sink.sink.state = SinkState::SendingParseOps;

                    let tok_internal_state = tokenizer.get_sendable();
                    self.html_tokenizer_sender.send(
                        MainThreadToParserThreadMessage::RestoreInternalState { tok_internal_state }
                    ).unwrap();
                    input.update_with_new_data(None);
                    assert!(self.pending_result.is_none());
                } else {
                    // happy case: We ran the script and document.write was not called. Great!
                    // We send the operations we speculatively parsed to the script thread.
                    self.html_tokenizer_sender.send(MainThreadToParserThreadMessage::FlushTreeOps).unwrap();
                    let response = self.receiver.recv().expect("Unexpected channel panic in main thread.");
                    let ParserThreadToMainThreadMessage::SpeculativeParseOps(speculative_operations) = response else {
                        panic!("parser thread sent unexpected response");
                    };
                    speculative_operations.into_iter().for_each(|operation| self.executor.process_operation(operation));

                    let (mut updated_input, result) = match msg {
                        ParserThreadToMainThreadMessage::TokenizerResultDone { updated_input, speculative_parsing_mode } => {
                            assert!(speculative_parsing_mode);
                            (updated_input, MutNullableDom::new(None))
                        },
                        ParserThreadToMainThreadMessage::TokenizerResultScript { script, updated_input, speculative_parsing_mode } => {
                            assert!(speculative_parsing_mode);
                            let script = self.executor.get_node(&script.id);
                            (updated_input, MutNullableDom::new(Some(script.downcast().unwrap())))
                        },
                        _ => unreachable!(),
                    };

                    let mut new_updated_input = VecDeque::new();
                    while let Some(st) = updated_input.pop_front() {
                        new_updated_input.push_back(StrTendril::from(st));
                    }
                    input.update_with_new_data(Some(new_updated_input));
                    self.pending_result = Some(result);
                }
            },
            TokenizerState::ExecutingParseOps => unreachable!(),
        }
    }
}

/// Entry point for the parser thread.
fn run(sink: SendableSink,
       fragment_context_is_some: bool,
       ctxt_parse_node: Option<ParseNode>,
       form_parse_node: Option<ParseNode>,
       sender: Sender<ParserThreadToMainThreadMessage>,
       receiver: Receiver<MainThreadToParserThreadMessage>) {

    // FIXME: We should probably receive these options from the main thread
    let options = TreeBuilderOpts {
        ignore_missing_rules: true,
        scripting_enabled,
        ..Default::default()
    };

    let mut sink = Sink::get_self_from_sendable(sink);
    sink.sender = Some(sender.clone());
    let mut html_tokenizer = if fragment_context_is_some {
        let tb = TreeBuilder::new_for_fragment(
            sink,
            ctxt_parse_node.unwrap(),
            form_parse_node,
            options);

        // FIXME: We should probably receive these options from the main thread
        let tok_options = TokenizerOpts {
            initial_state: Some(tb.tokenizer_state_for_context_elem()),
            ..Default::default()
        };

        HtmlTokenizer::new(tb, tok_options)
    } else {
        HtmlTokenizer::new(TreeBuilder::new(sink, options), Default::default())
    };

    loop {
        match receiver.recv().expect("Unexpected channel panic in html parser thread") {
            MainThreadToParserThreadMessage::Feed { input, should_parse_speculatively } => {
                let mut input = create_buffer_queue(input);
                if should_parse_speculatively {
                    let sendable_tokenizer = html_tokenizer.get_sendable();
                    html_tokenizer.sink.sink.state = SinkState::BufferingParseOperations(VecDeque::new());
                    sender.send(ParserThreadToMainThreadMessage::HtmlTokenizerInternalState(sendable_tokenizer)).unwrap();
                }
                let res = html_tokenizer.feed(&mut input);

                // Gather changes to 'input' and place them in 'updated_input',
                // which will be sent to the main thread to update feed method's 'input'
                let mut updated_input = VecDeque::new();
                while let Some(st) = input.pop_front() {
                    updated_input.push_back(SendTendril::from(st));
                }

                let res = match res {
                    TokenizerResult::Done => ParserThreadToMainThreadMessage::TokenizerResultDone {
                                                 updated_input,
                                                 speculative_parsing_mode: should_parse_speculatively
                                             },
                    TokenizerResult::Script(script) => ParserThreadToMainThreadMessage::TokenizerResultScript {
                                                           script,
                                                           updated_input,
                                                           speculative_parsing_mode: should_parse_speculatively
                                                       },
                };
                sender.send(res).unwrap();
            },
            MainThreadToParserThreadMessage::FlushTreeOps => {
                html_tokenizer.sink.sink.flush_tree_ops();
            }
            MainThreadToParserThreadMessage::RestoreInternalState { tok_internal_state } => {
                html_tokenizer = HtmlTokenizer::get_self_from_sendable(tok_internal_state);
                html_tokenizer.sink.sink.sender = Some(sender.clone());
            },
            MainThreadToParserThreadMessage::End => {
                html_tokenizer.end();
                sender.send(ParserThreadToMainThreadMessage::End).unwrap();
                break;
            },
            MainThreadToParserThreadMessage::SetPlainTextState => html_tokenizer.set_plaintext_state(),
        };
    }
}

#[derive(Clone, Default, JSTraceable, MallocSizeOf)]
struct ParseNodeData {
    contents: Option<ParseNode>,
    /// Whether this node is a mathml integration point (?).
    is_integration_point: bool,
}

pub struct SendableSink {
    current_line: u64,
    parse_node_data: HashMap<ParseNodeId, ParseNodeData>,
    next_parse_node_id: ParseNodeId,
    document_node: ParseNode,
}

#[derive(JSTraceable)]
enum SinkState {
    /// Default state of the Sink, sends all parse operations to main thread.
    SendingParseOps,
    /// State assumed while parsing document.write()'s contents on the main thread.
    ParsingDocWriteContents(ParseOperationExecutor),
    /// Speculative parsing mode, enqueues parse operations in parser thread.
    BufferingParseOperations(VecDeque<ParseOperation>),
}

/// A [TreeSink] impl that records the operations performed on it.
///
/// These are then sent to a [ParseOperationExecutor] and executed.
#[derive(JSTraceable)]
pub struct Sink {
    current_line: u64,
    parse_node_data: HashMap<ParseNodeId, ParseNodeData>,
    next_parse_node_id: Cell<ParseNodeId>,
    document_node: ParseNode,
    sender: Option<Sender<ParserThreadToMainThreadMessage>>,
    state: SinkState,
}

impl Sink {
    fn new(sender: Sender<ParserThreadToMainThreadMessage>) -> Sink {
        let sink = Sink {
            current_line: Cell::new(1),
            parse_node_data: RefCell::new(HashMap::new()),
            next_parse_node_id: Cell::new(1),
            document_node: ParseNode {
                id: 0,
                qual_name: None,
            },
            sender: Some(sender),
            state: SinkState::SendingParseOps,
        };
        let data = ParseNodeData::default();
        sink.insert_parse_node_data(0, data);
        sink
    }

    fn new_parse_node(&self) -> ParseNode {
        let id = self.next_parse_node_id.get();
        let data = ParseNodeData::default();
        self.insert_parse_node_data(id, data);
        self.next_parse_node_id.set(id + 1);
        ParseNode {
            id,
            qual_name: None,
        }
    }

    fn send_msg(&self, msg: ParserThreadToMainThreadMessage) {
        self.sender.as_ref().unwrap().send(msg).unwrap()
    }

    fn process_operation(&mut self, op: ParseOperation) {
        match self.state {
            SinkState::BufferingParseOperations(ref mut parse_op_queue) => parse_op_queue.push_back(op),
            SinkState::SendingParseOps => self.send_msg(ParserThreadToMainThreadMessage::ProcessOperation(op)),
            SinkState::ParsingDocWriteContents(ref mut executor) => executor.process_operation(op),
        }
    }

    /// Send all the queued parse operations to the main thread
    /// 
    /// ### Panics
    /// Panics if the sink is not currently speculatively parsing, meaning there are no buffered
    /// parse operations to flush.
    fn flush_tree_ops(&mut self) {
        let old_state = mem::replace(&mut self.state, SinkState::SendingParseOps);
        let SinkState::BufferingParseOperations(parse_op_queue) = old_state else {
            unreachable!();
        };

        self.send_msg(ParserThreadToMainThreadMessage::SpeculativeParseOps(parse_op_queue));
    }

    fn insert_parse_node_data(&self, id: ParseNodeId, data: ParseNodeData) {
        let previous = self.parse_node_data.borrow_mut().insert(id, data);
        assert!(previous.is_none(), "id already exists");
    }

    fn get_parse_node_data<'a>(&'a self, id: &'a ParseNodeId) -> Ref<'a, ParseNodeData> {
        Ref::map(self.parse_node_data.borrow(), |data| {
            data.get(id).expect("Parse Node data not found!")
        })
    }

    fn get_parse_node_data_mut<'a>(&'a self, id: &'a ParseNodeId) -> RefMut<'a, ParseNodeData> {
        RefMut::map(self.parse_node_data.borrow_mut(), |data| {
            data.get_mut(id).expect("Parse Node data not found!")
        })
    }
}

#[cfg_attr(crown, allow(crown::unrooted_must_root))]
impl TreeSink for Sink {
    type Output = Self;

    fn finish(self) -> Self {
        self
    }

    type Handle = ParseNode;
    type ElemName<'a>
        = ExpandedName<'a>
    where
        Self: 'a;

    fn get_document(&self) -> Self::Handle {
        self.document_node.clone()
    }

    fn get_template_contents(&self, target: &Self::Handle) -> Self::Handle {
        if let Some(ref contents) = self.get_parse_node_data(&target.id).contents {
            return contents.clone();
        }
        let node = self.new_parse_node();
        {
            let mut data = self.get_parse_node_data_mut(&target.id);
            data.contents = Some(node.clone());
        }
        self.process_operation(ParseOperation::GetTemplateContents { target: target.id, contents: node.id });
        node
    }

    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool {
        x.id == y.id
    }

    fn elem_name<'a>(&self, target: &'a Self::Handle) -> ExpandedName<'a> {
        target
            .qual_name
            .as_ref()
            .expect("Expected qual name of node!")
            .expanded()
    }

    fn create_element(
        &self,
        name: QualName,
        html_attrs: Vec<HtmlAttribute>,
        _flags: ElementFlags,
    ) -> Self::Handle {
        let mut node = self.new_parse_node();
        node.qual_name = Some(name.clone());
        {
            let mut node_data = self.get_parse_node_data_mut(&node.id);
            node_data.is_integration_point = html_attrs.iter().any(|attr| {
                let attr_value = &String::from(attr.value.clone());
                (attr.name.local == local_name!("encoding") && attr.name.ns == ns!()) &&
                    (attr_value.eq_ignore_ascii_case("text/html") ||
                        attr_value.eq_ignore_ascii_case("application/xhtml+xml"))
            });
        }
        let attrs = html_attrs
            .into_iter()
            .map(|attr| Attribute {
                name: attr.name,
                value: String::from(attr.value),
            })
            .collect();

        let current_line = self.current_line;
        self.process_op(ParseOperation::CreateElement {
            node: node.id,
            name,
            attrs,
            current_line: self.current_line.get(),
        });
        node
    }

    fn create_comment(&self, text: StrTendril) -> Self::Handle {
        let node = self.new_parse_node();
        self.process_operation(ParseOperation::CreateComment { text: String::from(text), node: node.id });
        node
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> ParseNode {
        let node = self.new_parse_node();
        self.process_op(ParseOperation::CreatePI {
            node: node.id,
            target: String::from(target),
            data: String::from(data),
        });
        node
    }

    fn associate_with_form(
        &self,
        target: &Self::Handle,
        form: &Self::Handle,
        nodes: (&Self::Handle, Option<&Self::Handle>),
    ) {
        let (element, prev_element) = nodes;
        self.process_op(ParseOperation::AssociateWithForm {
            target: target.id,
            form: form.id,
            element: element.id,
            prev_element: prev_element.map(|p| p.id),
        });
    }

    fn append_before_sibling(
        &self,
        sibling: &Self::Handle,
        new_node: HtmlNodeOrText<Self::Handle>,
    ) {
        let new_node = match new_node {
            HtmlNodeOrText::AppendNode(node) => NodeOrText::Node(node),
            HtmlNodeOrText::AppendText(text) => NodeOrText::Text(String::from(text)),
        };

        self.process_operation(ParseOperation::AppendBeforeSibling { sibling: sibling.id, node: new_node });
    }

    fn append_based_on_parent_node(
        &self,
        elem: &Self::Handle,
        prev_elem: &Self::Handle,
        child: HtmlNodeOrText<Self::Handle>,
    ) {
        let child = match child {
            HtmlNodeOrText::AppendNode(node) => NodeOrText::Node(node),
            HtmlNodeOrText::AppendText(text) => NodeOrText::Text(String::from(text)),
        };
        self.process_op(ParseOperation::AppendBasedOnParentNode {
            element: elem.id,
            prev_element: prev_elem.id,
            node: child,
        });
    }

    fn parse_error(&self, msg: Cow<'static, str>) {
        debug!("Parse error: {}", msg);
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        let mode = match mode {
            QuirksMode::Quirks => ServoQuirksMode::Quirks,
            QuirksMode::LimitedQuirks => ServoQuirksMode::LimitedQuirks,
            QuirksMode::NoQuirks => ServoQuirksMode::NoQuirks,
        };
        self.process_op(ParseOperation::SetQuirksMode { mode });
    }

    fn append(&self, parent: &Self::Handle, child: HtmlNodeOrText<Self::Handle>) {
        let child = match child {
            HtmlNodeOrText::AppendNode(node) => NodeOrText::Node(node),
            HtmlNodeOrText::AppendText(text) => NodeOrText::Text(String::from(text)),
        };
        self.process_operation(ParseOperation::Append { parent: parent.id, node: child });
    }

    fn append_doctype_to_document(&self, name: StrTendril, public_id: StrTendril,
                                  system_id: StrTendril) {
        self.process_operation(ParseOperation::AppendDoctypeToDocument {
            name: String::from(name),
            public_id: String::from(public_id),
            system_id: String::from(system_id)
        });
    }

    fn add_attrs_if_missing(&self, target: &Self::Handle, html_attrs: Vec<HtmlAttribute>) {
        let attrs = html_attrs.into_iter()
            .map(|attr| Attribute { name: attr.name, value: String::from(attr.value) }).collect();
        self.process_op(ParseOperation::AddAttrsIfMissing { target: target.id, attrs });
    }

    fn remove_from_parent(&self, target: &Self::Handle) {
        self.process_operation(ParseOperation::RemoveFromParent { target: target.id });
    }

    fn mark_script_already_started(&self, node: &Self::Handle) {
        self.process_operation(ParseOperation::MarkScriptAlreadyStarted { node: node.id });
    }

    fn reparent_children(&self, parent: &Self::Handle, new_parent: &Self::Handle) {
        self.process_operation(ParseOperation::ReparentChildren { parent: parent.id, new_parent: new_parent.id });
    }

    /// <https://html.spec.whatwg.org/multipage/#html-integration-point>
    ///
    /// Specifically, the `<annotation-xml>` cases.
    fn is_mathml_annotation_xml_integration_point(&self, handle: &Self::Handle) -> bool {
        let node_data = self.get_parse_node_data(&handle.id);
        node_data.is_integration_point
    }

    fn set_current_line(&self, line_number: u64) {
        self.current_line.set(line_number);
    }

    fn pop(&self, node: &Self::Handle) {
        self.process_operation(ParseOperation::Pop { node: node.id });
    }
}
