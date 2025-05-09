/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;
use std::time::{Duration, Instant};
use std::{cmp, thread};

use constellation_traits::EmbedderToConstellationMessage;
use embedder_traits::{MouseButtonAction, WebDriverCommandMsg, WebDriverScriptCommand};
use ipc_channel::ipc;
use keyboard_types::webdriver::KeyInputState;
use webdriver::actions::{
    ActionSequence, ActionsType, GeneralAction, KeyAction, KeyActionItem, KeyDownAction,
    KeyUpAction, NullActionItem, PointerAction, PointerActionItem, PointerActionParameters,
    PointerDownAction, PointerMoveAction, PointerOrigin, PointerType, PointerUpAction,
};
use webdriver::error::ErrorStatus;

use crate::Handler;

// Interval between pointerMove increments in ms, based on common vsync
static POINTERMOVE_INTERVAL: u64 = 17;

// https://w3c.github.io/webdriver/#dfn-input-source-state
pub(crate) enum InputSourceState {
    Null,
    Key(KeyInputState),
    Pointer(PointerInputState),
}

// https://w3c.github.io/webdriver/#dfn-pointer-input-source
pub(crate) struct PointerInputState {
    subtype: PointerType,
    pressed: HashSet<u64>,
    x: i64,
    y: i64,
}

impl PointerInputState {
    pub fn new(subtype: &PointerType) -> PointerInputState {
        PointerInputState {
            subtype: match subtype {
                PointerType::Mouse => PointerType::Mouse,
                PointerType::Pen => PointerType::Pen,
                PointerType::Touch => PointerType::Touch,
            },
            pressed: HashSet::new(),
            x: 0,
            y: 0,
        }
    }
}

// https://w3c.github.io/webdriver/#dfn-computing-the-tick-duration
fn compute_tick_duration(tick_actions: &ActionSequence) -> u64 {
    let mut duration = 0;
    match &tick_actions.actions {
        ActionsType::Null { actions } => {
            for action in actions.iter() {
                let NullActionItem::General(GeneralAction::Pause(pause_action)) = action;
                duration = cmp::max(duration, pause_action.duration.unwrap_or(0));
            }
        },
        ActionsType::Pointer {
            parameters: _,
            actions,
        } => {
            for action in actions.iter() {
                let action_duration = match action {
                    PointerActionItem::General(GeneralAction::Pause(action)) => action.duration,
                    PointerActionItem::Pointer(PointerAction::Move(action)) => action.duration,
                    _ => None,
                };
                duration = cmp::max(duration, action_duration.unwrap_or(0));
            }
        },
        ActionsType::Key { actions: _ } => (),
        ActionsType::Wheel { .. } => log::error!("not implemented"),
    }
    duration
}

impl Handler {
    // https://w3c.github.io/webdriver/#dfn-dispatch-actions
    pub(crate) fn dispatch_actions(
        &mut self,
        actions_by_tick: &[ActionSequence],
    ) -> Result<(), ErrorStatus> {
        for tick_actions in actions_by_tick.iter() {
            let tick_duration = compute_tick_duration(tick_actions);
            self.dispatch_tick_actions(tick_actions, tick_duration)?;
        }
        Ok(())
    }

    fn dispatch_general_action(&mut self, source_id: &str) {
        self.session_mut()
            .unwrap()
            .input_state_table
            .entry(source_id.to_string())
            .or_insert(InputSourceState::Null);
        // https://w3c.github.io/webdriver/#dfn-dispatch-a-pause-action
        // Nothing to be done
    }

    // https://w3c.github.io/webdriver/#dfn-dispatch-tick-actions
    fn dispatch_tick_actions(
        &mut self,
        tick_actions: &ActionSequence,
        tick_duration: u64,
    ) -> Result<(), ErrorStatus> {
        let source_id = &tick_actions.id;
        match &tick_actions.actions {
            ActionsType::Null { actions } => {
                for _action in actions.iter() {
                    self.dispatch_general_action(source_id);
                }
            },
            ActionsType::Key { actions } => {
                for action in actions.iter() {
                    match action {
                        KeyActionItem::General(_action) => {
                            self.dispatch_general_action(source_id);
                        },
                        KeyActionItem::Key(action) => {
                            self.session_mut()
                                .unwrap()
                                .input_state_table
                                .entry(source_id.to_string())
                                .or_insert(InputSourceState::Key(KeyInputState::new()));
                            match action {
                                KeyAction::Down(action) => {
                                    self.dispatch_keydown_action(source_id, action)
                                },
                                KeyAction::Up(action) => {
                                    self.dispatch_keyup_action(source_id, action)
                                },
                            };
                        },
                    }
                }
            },
            ActionsType::Pointer {
                parameters,
                actions,
            } => {
                for action in actions.iter() {
                    match action {
                        PointerActionItem::General(_action) => {
                            self.dispatch_general_action(source_id);
                        },
                        PointerActionItem::Pointer(action) => {
                            self.session_mut()
                                .unwrap()
                                .input_state_table
                                .entry(source_id.to_string())
                                .or_insert(InputSourceState::Pointer(PointerInputState::new(
                                    &parameters.pointer_type,
                                )));
                            match action {
                                PointerAction::Cancel => (),
                                PointerAction::Down(action) => {
                                    self.dispatch_pointerdown_action(source_id, action)
                                },
                                PointerAction::Move(action) => self.dispatch_pointermove_action(
                                    source_id,
                                    action,
                                    tick_duration,
                                )?,
                                PointerAction::Up(action) => {
                                    self.dispatch_pointerup_action(source_id, action)
                                },
                            }
                        },
                    }
                }
            },
            ActionsType::Wheel { .. } => {
                log::error!("not yet implemented");
                return Err(ErrorStatus::UnsupportedOperation);
            },
        }

        Ok(())
    }

    // https://w3c.github.io/webdriver/#dfn-dispatch-a-keydown-action
    fn dispatch_keydown_action(&mut self, source_id: &str, action: &KeyDownAction) {
        let session = self.session.as_mut().unwrap();

        let raw_key = action.value.chars().next().unwrap();
        let key_input_state = match session.input_state_table.get_mut(source_id).unwrap() {
            InputSourceState::Null => unreachable!(),
            InputSourceState::Key(key_input_state) => key_input_state,
            InputSourceState::Pointer(_) => unreachable!(),
        };

        session.input_cancel_list.push(ActionSequence {
            id: source_id.into(),
            actions: ActionsType::Key {
                actions: vec![KeyActionItem::Key(KeyAction::Up(KeyUpAction {
                    value: action.value.clone(),
                }))],
            },
        });

        let keyboard_event = key_input_state.dispatch_keydown(raw_key);
        let cmd_msg =
            WebDriverCommandMsg::KeyboardAction(session.browsing_context_id, keyboard_event);
        self.constellation_chan
            .send(EmbedderToConstellationMessage::WebDriverCommand(cmd_msg))
            .unwrap();
    }

    // https://w3c.github.io/webdriver/#dfn-dispatch-a-keyup-action
    fn dispatch_keyup_action(&mut self, source_id: &str, action: &KeyUpAction) {
        let session = self.session.as_mut().unwrap();

        let raw_key = action.value.chars().next().unwrap();
        let key_input_state = match session.input_state_table.get_mut(source_id).unwrap() {
            InputSourceState::Null => unreachable!(),
            InputSourceState::Key(key_input_state) => key_input_state,
            InputSourceState::Pointer(_) => unreachable!(),
        };

        session.input_cancel_list.push(ActionSequence {
            id: source_id.into(),
            actions: ActionsType::Key {
                actions: vec![KeyActionItem::Key(KeyAction::Up(KeyUpAction {
                    value: action.value.clone(),
                }))],
            },
        });

        if let Some(keyboard_event) = key_input_state.dispatch_keyup(raw_key) {
            let cmd_msg =
                WebDriverCommandMsg::KeyboardAction(session.browsing_context_id, keyboard_event);
            self.constellation_chan
                .send(EmbedderToConstellationMessage::WebDriverCommand(cmd_msg))
                .unwrap();
        }
    }

    // https://w3c.github.io/webdriver/#dfn-dispatch-a-pointerdown-action
    pub(crate) fn dispatch_pointerdown_action(
        &mut self,
        source_id: &str,
        action: &PointerDownAction,
    ) {
        let session = self.session.as_mut().unwrap();

        let pointer_input_state = match session.input_state_table.get_mut(source_id).unwrap() {
            InputSourceState::Null => unreachable!(),
            InputSourceState::Key(_) => unreachable!(),
            InputSourceState::Pointer(pointer_input_state) => pointer_input_state,
        };

        if pointer_input_state.pressed.contains(&action.button) {
            return;
        }
        pointer_input_state.pressed.insert(action.button);

        session.input_cancel_list.push(ActionSequence {
            id: source_id.into(),
            actions: ActionsType::Pointer {
                parameters: PointerActionParameters {
                    pointer_type: match pointer_input_state.subtype {
                        PointerType::Mouse => PointerType::Mouse,
                        PointerType::Pen => PointerType::Pen,
                        PointerType::Touch => PointerType::Touch,
                    },
                },
                actions: vec![PointerActionItem::Pointer(PointerAction::Up(
                    PointerUpAction {
                        button: action.button,
                        ..Default::default()
                    },
                ))],
            },
        });

        let button = (action.button as u16).into();
        let cmd_msg = WebDriverCommandMsg::MouseButtonAction(
            session.webview_id,
            MouseButtonAction::Down,
            button,
            pointer_input_state.x as f32,
            pointer_input_state.y as f32,
        );
        self.constellation_chan
            .send(EmbedderToConstellationMessage::WebDriverCommand(cmd_msg))
            .unwrap();
    }

    // https://w3c.github.io/webdriver/#dfn-dispatch-a-pointerup-action
    pub(crate) fn dispatch_pointerup_action(&mut self, source_id: &str, action: &PointerUpAction) {
        let session = self.session.as_mut().unwrap();

        let pointer_input_state = match session.input_state_table.get_mut(source_id).unwrap() {
            InputSourceState::Null => unreachable!(),
            InputSourceState::Key(_) => unreachable!(),
            InputSourceState::Pointer(pointer_input_state) => pointer_input_state,
        };

        if !pointer_input_state.pressed.contains(&action.button) {
            return;
        }
        pointer_input_state.pressed.remove(&action.button);

        session.input_cancel_list.push(ActionSequence {
            id: source_id.into(),
            actions: ActionsType::Pointer {
                parameters: PointerActionParameters {
                    pointer_type: match pointer_input_state.subtype {
                        PointerType::Mouse => PointerType::Mouse,
                        PointerType::Pen => PointerType::Pen,
                        PointerType::Touch => PointerType::Touch,
                    },
                },
                actions: vec![PointerActionItem::Pointer(PointerAction::Down(
                    PointerDownAction {
                        button: action.button,
                        ..Default::default()
                    },
                ))],
            },
        });

        let button = (action.button as u16).into();
        let cmd_msg = WebDriverCommandMsg::MouseButtonAction(
            session.webview_id,
            MouseButtonAction::Up,
            button,
            pointer_input_state.x as f32,
            pointer_input_state.y as f32,
        );
        self.constellation_chan
            .send(EmbedderToConstellationMessage::WebDriverCommand(cmd_msg))
            .unwrap();
    }

    // https://w3c.github.io/webdriver/#dfn-dispatch-a-pointermove-action
    pub(crate) fn dispatch_pointermove_action(
        &mut self,
        source_id: &str,
        action: &PointerMoveAction,
        tick_duration: u64,
    ) -> Result<(), ErrorStatus> {
        let tick_start = Instant::now();

        // Steps 1 - 2
        let x_offset = action.x;
        let y_offset = action.y;

        // Steps 3 - 4
        let (start_x, start_y) = match self
            .session
            .as_ref()
            .unwrap()
            .input_state_table
            .get(source_id)
            .unwrap()
        {
            InputSourceState::Null => unreachable!(),
            InputSourceState::Key(_) => unreachable!(),
            InputSourceState::Pointer(pointer_input_state) => {
                (pointer_input_state.x, pointer_input_state.y)
            },
        };

        // Step 5 - 6
        let (x, y) = match action.origin {
            PointerOrigin::Viewport => (x_offset, y_offset),
            PointerOrigin::Pointer => (start_x + x_offset, start_y + y_offset),
            PointerOrigin::Element(ref x) => {
                let (sender, receiver) = ipc::channel().unwrap();
                self.browsing_context_script_command(
                    WebDriverScriptCommand::GetElementInViewCenterPoint(x.to_string(), sender),
                )
                .unwrap();

                let Some(point) = receiver.recv().unwrap()? else {
                    return Err(ErrorStatus::UnknownError);
                };
                point
            },
        };

        let (sender, receiver) = ipc::channel().unwrap();
        let cmd_msg =
            WebDriverCommandMsg::GetWindowSize(self.session.as_ref().unwrap().webview_id, sender);
        self.constellation_chan
            .send(EmbedderToConstellationMessage::WebDriverCommand(cmd_msg))
            .unwrap();

        // Steps 7 - 8
        let viewport_size = receiver.recv().unwrap();
        if x < 0 || x as f32 > viewport_size.width || y < 0 || y as f32 > viewport_size.height {
            return Err(ErrorStatus::MoveTargetOutOfBounds);
        }

        // Step 9
        let duration = match action.duration {
            Some(duration) => duration,
            None => tick_duration,
        };

        // Step 10
        if duration > 0 {
            thread::sleep(Duration::from_millis(POINTERMOVE_INTERVAL));
        }

        // Step 11
        self.perform_pointer_move(source_id, duration, start_x, start_y, x, y, tick_start);

        // Step 12
        Ok(())
    }

    /// <https://w3c.github.io/webdriver/#dfn-perform-a-pointer-move>
    #[allow(clippy::too_many_arguments)]
    fn perform_pointer_move(
        &mut self,
        source_id: &str,
        duration: u64,
        start_x: i64,
        start_y: i64,
        target_x: i64,
        target_y: i64,
        tick_start: Instant,
    ) {
        let session = self.session.as_mut().unwrap();
        let pointer_input_state = match session.input_state_table.get_mut(source_id).unwrap() {
            InputSourceState::Null => unreachable!(),
            InputSourceState::Key(_) => unreachable!(),
            InputSourceState::Pointer(pointer_input_state) => pointer_input_state,
        };

        loop {
            // Step 1
            let time_delta = tick_start.elapsed().as_millis();

            // Step 2
            let duration_ratio = if duration > 0 {
                time_delta as f64 / duration as f64
            } else {
                1.0
            };

            // Step 3
            let last = 1.0 - duration_ratio < 0.001;

            // Step 4
            let (x, y) = if last {
                (target_x, target_y)
            } else {
                (
                    (duration_ratio * (target_x - start_x) as f64) as i64 + start_x,
                    (duration_ratio * (target_y - start_y) as f64) as i64 + start_y,
                )
            };

            // Steps 5 - 6
            let current_x = pointer_input_state.x;
            let current_y = pointer_input_state.y;

            // Step 7
            if x != current_x || y != current_y {
                // Step 7.2
                let cmd_msg =
                    WebDriverCommandMsg::MouseMoveAction(session.webview_id, x as f32, y as f32);
                self.constellation_chan
                    .send(EmbedderToConstellationMessage::WebDriverCommand(cmd_msg))
                    .unwrap();
                // Step 7.3
                pointer_input_state.x = x;
                pointer_input_state.y = y;
            }

            // Step 8
            if last {
                return;
            }

            // Step 9
            thread::sleep(Duration::from_millis(POINTERMOVE_INTERVAL));
        }
    }
}
