// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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

use super::consts;
use crate::keymap;
use shared::log;

use crate::window::AppWindow;
use rdp::sys::{
    PTR_FLAGS_BUTTON1,
    PTR_FLAGS_BUTTON2,
    PTR_FLAGS_BUTTON3,
    PTR_FLAGS_DOWN,
    PTR_FLAGS_MOVE,
    PTR_FLAGS_WHEEL,
    PTR_FLAGS_WHEEL_NEGATIVE,

    PTR_XFLAGS_BUTTON1,
    PTR_XFLAGS_BUTTON2,
    freerdp_input_send_extended_mouse_event,
    // SetEvent,
    freerdp_input_send_keyboard_event_ex,
    freerdp_input_send_mouse_event,
    rdpInput,
};



impl AppWindow {
    fn handle_mouse(
        &mut self,
        _ctx: &egui::Context,
        rdp_input: *mut rdpInput,
        input_state: &egui::InputState,
        scale: egui::Vec2,
    ) {
        for ev in &input_state.events {
            match ev {
                egui::Event::PointerMoved(pos) => {
                    // Mouse moved
                    let x = (pos.x * scale.x) as u16;
                    let y = (pos.y * scale.y) as u16;
                    unsafe {
                        freerdp_input_send_mouse_event(rdp_input, PTR_FLAGS_MOVE as u16, x, y)
                    };
                }
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    ..
                } => {
                    let (flags, xflags, is_down) = match button {
                        egui::PointerButton::Primary => (PTR_FLAGS_BUTTON1, 0, pressed.to_owned()),
                        egui::PointerButton::Secondary => {
                            (PTR_FLAGS_BUTTON2, 0, pressed.to_owned())
                        }
                        egui::PointerButton::Middle => (PTR_FLAGS_BUTTON3, 0, pressed.to_owned()),
                        egui::PointerButton::Extra1 => (0, PTR_XFLAGS_BUTTON1, pressed.to_owned()),
                        egui::PointerButton::Extra2 => (0, PTR_XFLAGS_BUTTON2, pressed.to_owned()),
                    };
                    let (x, y) = ((pos.x * scale.x) as u16, (pos.y * scale.y) as u16);
                    if flags != 0 {
                        unsafe {
                            freerdp_input_send_mouse_event(
                                rdp_input,
                                flags as u16 | if is_down { PTR_FLAGS_DOWN as u16 } else { 0 },
                                x,
                                y,
                            );
                        }
                    } else if xflags != 0 {
                        unsafe {
                            freerdp_input_send_extended_mouse_event(
                                rdp_input,
                                xflags as u16 | if is_down { PTR_FLAGS_DOWN as u16 } else { 0 },
                                x,
                                y,
                            );
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
                        // Convert negative deltas to 9bit two's complement
                        let cflags = if flags & (PTR_FLAGS_WHEEL_NEGATIVE as u16) != 0 {
                            flags | (0x100 - step)
                        } else {
                            flags | step
                        };
                        unsafe { freerdp_input_send_mouse_event(rdp_input, cflags, 0, 0) };
                    }
                }
                _ => { /* other events */ }
            }
        }
    }

    fn handle_keyboard(
        &mut self,
        _ctx: &egui::Context,
        rdp_input: *mut rdpInput,
        _input_state: &egui::InputState,
    ) {
        while let Ok(raw_key) = self.events.try_recv() {
            if let Some(scancode) = keymap::RdpScanCode::get_from_key(Some(&raw_key.keycode)) {
                unsafe {
                    freerdp_input_send_keyboard_event_ex(
                        rdp_input,
                        raw_key.pressed.into(),
                        raw_key.repeat.into(),
                        scancode as u32,
                    );
                };
            } else {
                log::debug!(
                    "No scancode mapping for keycode={:?}, pressed={}",
                    raw_key.keycode,
                    raw_key.pressed
                );
            }
        }
    }

    pub fn handle_input(
        &mut self,
        ctx: &egui::Context,
        rdp_input: *mut rdpInput,
        scale: egui::Vec2,
    ) {
        ctx.input(|input_state| {
            // // Log events for debugging
            // for ev in &input_state.events {
            //     log::debug!("Input event: {:?}", ev);
            // }
            // Handle mouse input
            self.handle_mouse(ctx, rdp_input, input_state, scale);
            // Handle keyboard input
            self.handle_keyboard(ctx, rdp_input, input_state);
        });
    }
}
