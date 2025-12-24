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
#![allow(dead_code)]
use std::{collections::HashMap, sync::LazyLock};

use super::scancodes::RdpScanCode;
use winit::keyboard::KeyCode;

/// Mapping table SDL → RDP
static _SCANCODE_LIST: &[(KeyCode, RdpScanCode)] = &[
    (KeyCode::KeyA, RdpScanCode::KeyA),
    (KeyCode::KeyB, RdpScanCode::KeyB),
    (KeyCode::KeyC, RdpScanCode::KeyC),
    (KeyCode::KeyD, RdpScanCode::KeyD),
    (KeyCode::KeyE, RdpScanCode::KeyE),
    (KeyCode::KeyF, RdpScanCode::KeyF),
    (KeyCode::KeyG, RdpScanCode::KeyG),
    (KeyCode::KeyH, RdpScanCode::KeyH),
    (KeyCode::KeyI, RdpScanCode::KeyI),
    (KeyCode::KeyJ, RdpScanCode::KeyJ),
    (KeyCode::KeyK, RdpScanCode::KeyK),
    (KeyCode::KeyL, RdpScanCode::KeyL),
    (KeyCode::KeyM, RdpScanCode::KeyM),
    (KeyCode::KeyN, RdpScanCode::KeyN),
    (KeyCode::KeyO, RdpScanCode::KeyO),
    (KeyCode::KeyP, RdpScanCode::KeyP),
    (KeyCode::KeyQ, RdpScanCode::KeyQ),
    (KeyCode::KeyR, RdpScanCode::KeyR),
    (KeyCode::KeyS, RdpScanCode::KeyS),
    (KeyCode::KeyT, RdpScanCode::KeyT),
    (KeyCode::KeyU, RdpScanCode::KeyU),
    (KeyCode::KeyV, RdpScanCode::KeyV),
    (KeyCode::KeyW, RdpScanCode::KeyW),
    (KeyCode::KeyX, RdpScanCode::KeyX),
    (KeyCode::KeyY, RdpScanCode::KeyY),
    (KeyCode::KeyZ, RdpScanCode::KeyZ),
    (KeyCode::Digit1, RdpScanCode::Key1),
    (KeyCode::Digit2, RdpScanCode::Key2),
    (KeyCode::Digit3, RdpScanCode::Key3),
    (KeyCode::Digit4, RdpScanCode::Key4),
    (KeyCode::Digit5, RdpScanCode::Key5),
    (KeyCode::Digit6, RdpScanCode::Key6),
    (KeyCode::Digit7, RdpScanCode::Key7),
    (KeyCode::Digit8, RdpScanCode::Key8),
    (KeyCode::Digit9, RdpScanCode::Key9),
    (KeyCode::Digit0, RdpScanCode::Key0),
    (KeyCode::Enter, RdpScanCode::Return),
    (KeyCode::Escape, RdpScanCode::Escape),
    (KeyCode::Backspace, RdpScanCode::Backspace),
    (KeyCode::Tab, RdpScanCode::Tab),
    (KeyCode::Space, RdpScanCode::Space),
    (KeyCode::Minus, RdpScanCode::OemMinus),
    (KeyCode::F1, RdpScanCode::F1),
    (KeyCode::F2, RdpScanCode::F2),
    (KeyCode::F3, RdpScanCode::F3),
    (KeyCode::F4, RdpScanCode::F4),
    (KeyCode::F5, RdpScanCode::F5),
    (KeyCode::F6, RdpScanCode::F6),
    (KeyCode::F7, RdpScanCode::F7),
    (KeyCode::F8, RdpScanCode::F8),
    (KeyCode::F9, RdpScanCode::F9),
    (KeyCode::F10, RdpScanCode::F10),
    (KeyCode::F11, RdpScanCode::F11),
    (KeyCode::F12, RdpScanCode::F12),
    (KeyCode::F13, RdpScanCode::F13),
    (KeyCode::F14, RdpScanCode::F14),
    (KeyCode::F15, RdpScanCode::F15),
    (KeyCode::F16, RdpScanCode::F16),
    (KeyCode::F17, RdpScanCode::F17),
    (KeyCode::F18, RdpScanCode::F18),
    (KeyCode::F19, RdpScanCode::F19),
    (KeyCode::F20, RdpScanCode::F20),
    (KeyCode::F21, RdpScanCode::F21),
    (KeyCode::F22, RdpScanCode::F22),
    (KeyCode::F23, RdpScanCode::F23),
    (KeyCode::F24, RdpScanCode::F24),
    (KeyCode::Comma, RdpScanCode::OemComma),
    (KeyCode::Period, RdpScanCode::OemPeriod),
    (KeyCode::Slash, RdpScanCode::Oem2),
    (KeyCode::Backslash, RdpScanCode::Oem5),
    (KeyCode::Insert, RdpScanCode::Insert),
    (KeyCode::Home, RdpScanCode::Home),
    (KeyCode::Delete, RdpScanCode::Delete),
    (KeyCode::ArrowRight, RdpScanCode::Right),
    (KeyCode::ArrowLeft, RdpScanCode::Left),
    (KeyCode::ArrowDown, RdpScanCode::Down),
    (KeyCode::ArrowUp, RdpScanCode::Up),
    (KeyCode::Semicolon, RdpScanCode::Oem1),
    (KeyCode::PageUp, RdpScanCode::Prior),
    (KeyCode::End, RdpScanCode::End),
    (KeyCode::PageDown, RdpScanCode::Next),
    // unmapped keys
    (KeyCode::Cut, RdpScanCode::Unknown),
    (KeyCode::Copy, RdpScanCode::Unknown),
    (KeyCode::Paste, RdpScanCode::Unknown),
    // additional mappings
    (KeyCode::AltLeft, RdpScanCode::LMenu),
    (KeyCode::AltRight, RdpScanCode::RMenu),
    (KeyCode::ControlLeft, RdpScanCode::LControl),
    (KeyCode::ControlRight, RdpScanCode::RControl),
    (KeyCode::ShiftLeft, RdpScanCode::LShift),
    (KeyCode::ShiftRight, RdpScanCode::RShift),
    (KeyCode::SuperLeft, RdpScanCode::LWin),
    (KeyCode::SuperRight, RdpScanCode::RWin),
    (KeyCode::CapsLock, RdpScanCode::CapsLock),
    (KeyCode::Backquote, RdpScanCode::Oem3),
    (KeyCode::BracketLeft, RdpScanCode::Oem4),
    (KeyCode::BracketRight, RdpScanCode::Oem6),
    (KeyCode::Quote, RdpScanCode::Oem7),
    (KeyCode::Equal, RdpScanCode::OemPlus),
    (KeyCode::IntlBackslash, RdpScanCode::Oem102),
    (KeyCode::IntlRo, RdpScanCode::Oem102),
    (KeyCode::IntlYen, RdpScanCode::Oem5),
    (KeyCode::NumLock, RdpScanCode::NumLock),
    (KeyCode::Numpad0, RdpScanCode::Numpad0),
    (KeyCode::Numpad1, RdpScanCode::Numpad1),
    (KeyCode::Numpad2, RdpScanCode::Numpad2),
    (KeyCode::Numpad3, RdpScanCode::Numpad3),
    (KeyCode::Numpad4, RdpScanCode::Numpad4),
    (KeyCode::Numpad5, RdpScanCode::Numpad5),
    (KeyCode::Numpad6, RdpScanCode::Numpad6),
    (KeyCode::Numpad7, RdpScanCode::Numpad7),
    (KeyCode::Numpad8, RdpScanCode::Numpad8),
    (KeyCode::Numpad9, RdpScanCode::Numpad9),
    (KeyCode::NumpadAdd, RdpScanCode::Add),
    (KeyCode::NumpadSubtract, RdpScanCode::Subtract),
    (KeyCode::NumpadMultiply, RdpScanCode::Multiply),
    (KeyCode::NumpadDivide, RdpScanCode::Divide),
    (KeyCode::NumpadDecimal, RdpScanCode::Decimal),
    (KeyCode::NumpadEnter, RdpScanCode::ReturnKP),
    (KeyCode::PrintScreen, RdpScanCode::PrintScreen),
    (KeyCode::Pause, RdpScanCode::Pause),
    (KeyCode::ScrollLock, RdpScanCode::ScrollLock),
    (KeyCode::ContextMenu, RdpScanCode::Apps),
    (KeyCode::Help, RdpScanCode::Help),
    (KeyCode::Convert, RdpScanCode::ConvertJp),
    (KeyCode::NonConvert, RdpScanCode::NonconvertJp),
    (KeyCode::KanaMode, RdpScanCode::Hiragana),
    (KeyCode::Lang1, RdpScanCode::HanjaKanji),
    (KeyCode::Lang2, RdpScanCode::KanaHangul),
];

// Hashed map for faster lookup
static WININIT_SCANCODE_MAP: LazyLock<HashMap<KeyCode, RdpScanCode>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for (sdl_scancode, rdp_scancode) in _SCANCODE_LIST.iter() {
        map.insert(*sdl_scancode, *rdp_scancode);
    }
    map
});

impl RdpScanCode {
    pub fn from_egui_key(key_code: &KeyCode) -> Option<Self> {
        WININIT_SCANCODE_MAP.get(key_code).copied()
    }
}
