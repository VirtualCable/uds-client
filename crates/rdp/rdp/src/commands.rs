// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
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
//
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use zeroize::Zeroize;

#[derive(Debug, Clone, Zeroize)]
pub enum InputEvent {
    Keyboard {
        scancode: u16,
        pressed: bool,
        repeat: bool,
    },
    Mouse {
        flags: u16,
        x: u16,
        y: u16,
    },
    ExtendedMouse {
        flags: u16,
        x: u16,
        y: u16,
    },
    Unicode {
        code: u16,
    },
}

#[derive(Debug, Clone)]
pub enum RdpCommand {
    // uds-client commands
    Input(InputEvent),
    ViewportMove {
        window_id: u32,
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    },
    LaunchRailApp {
        app: String,
        args: String,
        dir: String,
    },
    Close,

    // rdphtml5 commands
    Keyboard {
        is_down: bool,
        repeat: bool,
        scancode: u32,
    },
    Mouse {
        flags: u16,
        x: u16,
        y: u16,
    },
    Resize {
        width: u32,
        height: u32,
    },
    FocusIn,
}

pub type Sender = flume::Sender<RdpCommand>;
pub type Receiver = flume::Receiver<RdpCommand>;
