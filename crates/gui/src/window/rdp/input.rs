// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use eframe::egui;
use flume::Receiver;
use shared::log;

use super::consts;
use crate::{
    keymap,
    window::{rdp::connection::RdpConnectionState, types::HotKey},
};

use crate::window::AppWindow;
use rdp::sys::{
    PTR_FLAGS_BUTTON1, PTR_FLAGS_BUTTON2, PTR_FLAGS_BUTTON3, PTR_FLAGS_DOWN, PTR_FLAGS_MOVE,
    PTR_FLAGS_WHEEL, PTR_FLAGS_WHEEL_NEGATIVE, PTR_XFLAGS_BUTTON1, PTR_XFLAGS_BUTTON2,
};

#[allow(clippy::too_many_arguments)]
pub fn handle_mouse(
    _ctx: &egui::Context,
    command_tx: &rdp::commands::Sender,
    command_event: &rdp::utils::SafeHandle,
    input_state: &egui::InputState,
    scale: egui::Vec2,
    offset: egui::Vec2,
    desktop_size: (u32, u32),
    mut on_click: Option<&mut dyn FnMut()>,
) {
    for ev in &input_state.events {
        match ev {
            egui::Event::PointerMoved(pos) => {
                let x = ((pos.x + offset.x) * scale.x) as f64;
                let y = ((pos.y + offset.y) * scale.y) as f64;
                let x = x.clamp(0.0, desktop_size.0 as f64) as u16;
                let y = y.clamp(0.0, desktop_size.1 as f64) as u16;

                let _ = command_tx.send(rdp::commands::RdpCommand::Input(
                    rdp::commands::InputEvent::Mouse {
                        flags: PTR_FLAGS_MOVE as u16,
                        x,
                        y,
                    },
                ));
                unsafe {
                    rdp::sys::SetEvent(command_event.as_handle());
                }
            }
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                ..
            } => {
                if *pressed && let Some(on_click) = on_click.as_mut() {
                    on_click();
                }
                let (flags, xflags, is_down) = match button {
                    egui::PointerButton::Primary => (PTR_FLAGS_BUTTON1, 0, pressed.to_owned()),
                    egui::PointerButton::Secondary => (PTR_FLAGS_BUTTON2, 0, pressed.to_owned()),
                    egui::PointerButton::Middle => (PTR_FLAGS_BUTTON3, 0, pressed.to_owned()),
                    egui::PointerButton::Extra1 => (0, PTR_XFLAGS_BUTTON1, pressed.to_owned()),
                    egui::PointerButton::Extra2 => (0, PTR_XFLAGS_BUTTON2, pressed.to_owned()),
                };

                let x = (((pos.x + offset.x) * scale.x) as f64).clamp(0.0, desktop_size.0 as f64)
                    as u16;
                let y = (((pos.y + offset.y) * scale.y) as f64).clamp(0.0, desktop_size.1 as f64)
                    as u16;

                if flags != 0 {
                    let _ = command_tx.send(rdp::commands::RdpCommand::Input(
                        rdp::commands::InputEvent::Mouse {
                            flags: flags as u16 | if is_down { PTR_FLAGS_DOWN as u16 } else { 0 },
                            x,
                            y,
                        },
                    ));
                    unsafe {
                        rdp::sys::SetEvent(command_event.as_handle());
                    }
                } else if xflags != 0 {
                    let _ = command_tx.send(rdp::commands::RdpCommand::Input(
                        rdp::commands::InputEvent::ExtendedMouse {
                            flags: xflags as u16 | if is_down { PTR_FLAGS_DOWN as u16 } else { 0 },
                            x,
                            y,
                        },
                    ));
                    unsafe {
                        rdp::sys::SetEvent(command_event.as_handle());
                    }
                }
            }
            egui::Event::MouseWheel { unit, delta, .. } => {
                let mut wheel_delta = (match unit {
                    egui::MouseWheelUnit::Line => delta.y * consts::MOUSE_WHEEL_DELTA,
                    egui::MouseWheelUnit::Page => delta.y * (consts::MOUSE_WHEEL_DELTA * 10.0),
                    egui::MouseWheelUnit::Point => delta.y, // Not typical for mouse wheels
                }) as i32;

                let flags = (PTR_FLAGS_WHEEL
                    | if wheel_delta < 0 {
                        wheel_delta = -wheel_delta;
                        PTR_FLAGS_WHEEL_NEGATIVE
                    } else {
                        0
                    }) as u16;

                while wheel_delta > 0 {
                    let step: u16 = if wheel_delta > 0xFF {
                        0xFF
                    } else {
                        (wheel_delta & 0xFF) as u16
                    };
                    wheel_delta -= step as i32;
                    let cflags = if flags & (PTR_FLAGS_WHEEL_NEGATIVE as u16) != 0 {
                        flags | (0x100 - step)
                    } else {
                        flags | step
                    };
                    let _ = command_tx.send(rdp::commands::RdpCommand::Input(
                        rdp::commands::InputEvent::Mouse {
                            flags: cflags,
                            x: 0,
                            y: 0,
                        },
                    ));
                    unsafe {
                        rdp::sys::SetEvent(command_event.as_handle());
                    }
                }
            }
            _ => {}
        }
    }
}

impl AppWindow {
    pub(crate) fn handle_input(
        &mut self,
        rdp_state: &mut RdpConnectionState,
        ui: &mut egui::Ui,
        scale: egui::Vec2,
        offset: egui::Vec2,
    ) {
        let rdp_state_cloned = rdp_state.clone();
        ui.ctx().input(|input_state| {
            handle_mouse(
                ui.ctx(),
                &rdp_state_cloned.command_tx,
                &rdp_state_cloned.command_event,
                input_state,
                scale,
                offset,
                rdp_state_cloned.desktop_size,
                None,
            );
        });
        let input_state = ui.ctx().input(|i| i.clone());
        handle_keyboard(
            ui.ctx(),
            &rdp_state_cloned.command_tx,
            &rdp_state_cloned.command_event,
            &input_state,
            &self.keys_rx,
            Some(&rdp_state_cloned),
        );
    }
}

pub fn handle_keyboard(
    ctx: &egui::Context,
    command_tx: &rdp::commands::Sender,
    command_event: &rdp::utils::SafeHandle,
    input_state: &egui::InputState,
    keys_rx: &Receiver<crate::RawKey>,
    rdp_state: Option<&RdpConnectionState>,
) {
    if let Some(rdp_state) = rdp_state
        && !rdp_state.is_rail
    {
        // Process egui events ONLY for hotkeys
        for ev in &input_state.events {
            if let egui::Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
                ..
            } = ev
            {
                if *repeat {
                    continue;
                }
                let hotkey = HotKey::from_event(*key, *pressed, modifiers);
                match hotkey {
                    HotKey::ToggleFullScreen => {
                        rdp_state.toggle_fullscreen(ctx);
                    }
                    HotKey::ToggleFPS => {
                        rdp_state.fps.borrow_mut().toggle();
                    }
                    HotKey::Skip | HotKey::None => {}
                }
            }
        }
    }

    let is_focused = input_state.viewport().focused.unwrap_or(false);
    if !is_focused {
        return;
    }

    // Process raw keys for RDP
    while let Ok(raw_key) = keys_rx.try_recv() {
        if let Some(scancode) = keymap::RdpScanCode::get_from_key(Some(&raw_key.keycode)) {
            let _ = command_tx.send(rdp::commands::RdpCommand::Input(
                rdp::commands::InputEvent::Keyboard {
                    scancode: scancode as u16,
                    pressed: raw_key.pressed,
                },
            ));
            unsafe {
                rdp::sys::SetEvent(command_event.as_handle());
            }
        } else {
            log::debug!(
                "No scancode mapping for keycode={:?}, pressed={}",
                raw_key.keycode,
                raw_key.pressed
            );
        }
    }
}
