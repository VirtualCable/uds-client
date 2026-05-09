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
use winit::keyboard::KeyCode;

mod scancodes;
mod winit_keys;

pub use scancodes::RdpScanCode;

impl RdpScanCode {
    pub fn get_from_key(key: Option<&KeyCode>) -> Option<Self> {
        if let Some(k) = key {
            RdpScanCode::from_winit_key(k)
        } else {
            None
        }
    }
}

pub fn egui_to_scancode(key: egui::Key) -> Vec<u16> {
    let scancode = match key {
        egui::Key::A => RdpScanCode::KeyA,
        egui::Key::B => RdpScanCode::KeyB,
        egui::Key::C => RdpScanCode::KeyC,
        egui::Key::D => RdpScanCode::KeyD,
        egui::Key::E => RdpScanCode::KeyE,
        egui::Key::F => RdpScanCode::KeyF,
        egui::Key::G => RdpScanCode::KeyG,
        egui::Key::H => RdpScanCode::KeyH,
        egui::Key::I => RdpScanCode::KeyI,
        egui::Key::J => RdpScanCode::KeyJ,
        egui::Key::K => RdpScanCode::KeyK,
        egui::Key::L => RdpScanCode::KeyL,
        egui::Key::M => RdpScanCode::KeyM,
        egui::Key::N => RdpScanCode::KeyN,
        egui::Key::O => RdpScanCode::KeyO,
        egui::Key::P => RdpScanCode::KeyP,
        egui::Key::Q => RdpScanCode::KeyQ,
        egui::Key::R => RdpScanCode::KeyR,
        egui::Key::S => RdpScanCode::KeyS,
        egui::Key::T => RdpScanCode::KeyT,
        egui::Key::U => RdpScanCode::KeyU,
        egui::Key::V => RdpScanCode::KeyV,
        egui::Key::W => RdpScanCode::KeyW,
        egui::Key::X => RdpScanCode::KeyX,
        egui::Key::Y => RdpScanCode::KeyY,
        egui::Key::Z => RdpScanCode::KeyZ,
        egui::Key::Num0 => RdpScanCode::Key0,
        egui::Key::Num1 => RdpScanCode::Key1,
        egui::Key::Num2 => RdpScanCode::Key2,
        egui::Key::Num3 => RdpScanCode::Key3,
        egui::Key::Num4 => RdpScanCode::Key4,
        egui::Key::Num5 => RdpScanCode::Key5,
        egui::Key::Num6 => RdpScanCode::Key6,
        egui::Key::Num7 => RdpScanCode::Key7,
        egui::Key::Num8 => RdpScanCode::Key8,
        egui::Key::Num9 => RdpScanCode::Key9,
        egui::Key::Enter => RdpScanCode::Return,
        egui::Key::Escape => RdpScanCode::Escape,
        egui::Key::Backspace => RdpScanCode::Backspace,
        egui::Key::Tab => RdpScanCode::Tab,
        egui::Key::Space => RdpScanCode::Space,
        egui::Key::F1 => RdpScanCode::F1,
        egui::Key::F2 => RdpScanCode::F2,
        egui::Key::F3 => RdpScanCode::F3,
        egui::Key::F4 => RdpScanCode::F4,
        egui::Key::F5 => RdpScanCode::F5,
        egui::Key::F6 => RdpScanCode::F6,
        egui::Key::F7 => RdpScanCode::F7,
        egui::Key::F8 => RdpScanCode::F8,
        egui::Key::F9 => RdpScanCode::F9,
        egui::Key::F10 => RdpScanCode::F10,
        egui::Key::F11 => RdpScanCode::F11,
        egui::Key::F12 => RdpScanCode::F12,
        egui::Key::Insert => RdpScanCode::Insert,
        egui::Key::Home => RdpScanCode::Home,
        egui::Key::Delete => RdpScanCode::Delete,
        egui::Key::End => RdpScanCode::End,
        egui::Key::PageDown => RdpScanCode::Next,
        egui::Key::PageUp => RdpScanCode::Prior,
        egui::Key::ArrowLeft => RdpScanCode::Left,
        egui::Key::ArrowRight => RdpScanCode::Right,
        egui::Key::ArrowUp => RdpScanCode::Up,
        egui::Key::ArrowDown => RdpScanCode::Down,
        _ => RdpScanCode::Unknown,
    };

    if scancode == RdpScanCode::Unknown {
        vec![]
    } else {
        vec![scancode as u16]
    }
}
