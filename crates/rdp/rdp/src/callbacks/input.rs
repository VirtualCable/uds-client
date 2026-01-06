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
use freerdp_sys::{INT16, UINT8, UINT16, UINT32};

use shared::log::debug;

pub trait InputCallbacks {
    fn on_synchronize_event(&mut self, flags: UINT32) -> bool {
        debug!("Synchronize event: flags={}", flags);
        true
    }

    fn on_keyboard_event(&mut self, flags: UINT16, code: UINT8) -> bool {
        debug!("Keyboard event: flags={}, code={}", flags, code);
        true
    }

    fn on_unicode_keyboard_event(&mut self, flags: UINT16, code: UINT16) -> bool {
        debug!("Unicode keyboard event: flags={}, code={}", flags, code);
        true
    }

    fn on_mouse_event(&mut self, flags: UINT16, x: UINT16, y: UINT16) -> bool {
        debug!("Mouse event: flags={}, x={}, y={}", flags, x, y);
        true
    }

    fn on_extended_mouse_event(&mut self, flags: UINT16, x: UINT16, y: UINT16) -> bool {
        debug!("Extended mouse event: flags={}, x={}, y={}", flags, x, y);
        true
    }

    fn on_focus_in_event(&mut self, toggle_states: UINT16) -> bool {
        debug!("Focus in event: toggle_states={}", toggle_states);
        true
    }

    fn on_keyboard_pause_event(&mut self) -> bool {
        debug!("Keyboard pause event");
        true
    }

    fn on_rel_mouse_event(&mut self, flags: UINT16, x: INT16, y: INT16) -> bool {
        debug!("Relative mouse event: flags={}, x={}, y={}", flags, x, y);
        true
    }

    fn on_qoe_event(&mut self, flags: UINT32) -> bool {
        debug!("QoE event: flags={}", flags);
        true
    }
}
