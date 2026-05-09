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
use flume;

use crate::geom::Rect;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum RdpMessage {
    UpdateRects(Vec<Rect>),
    Disconnect,
    FocusRequired,
    Error(String),
    SetCursorIcon(Vec<u8>, u32, u32, u32, u32), // x, y, (of pointer "pointer") width, height
    ClipboardData(String),
    None, // Used on interrrupting recv by a timeout

    // RAIL window events — metadata only, no pixel routing
    // show_state: None = unknown, Some(2) = minimized, Some(3) = maximized, Some(1/5) = normal
    WindowCreate {
        window_id: u32,
        owner_id: Option<u32>,
        style: Option<u32>,
        ext_style: Option<u32>,
        taskbar_button: Option<bool>,
        title: String,
        show_state: Option<u8>,
        is_offscreen: Option<bool>,
        pos: Option<(i32, i32)>,
        size: Option<(u32, u32)>,
    },
    WindowUpdate {
        window_id: u32,
        owner_id: Option<u32>,
        style: Option<u32>,
        ext_style: Option<u32>,
        taskbar_button: Option<bool>,
        title: String,
        show_state: Option<u8>,
        /// Some(true) when coordinates are in the offscreen/minimized zone (< -1000)
        /// None when field flags don't specify coordinate updates
        is_offscreen: Option<bool>,
        pos: Option<(i32, i32)>,
        size: Option<(u32, u32)>,
    },
    WindowDelete(u32),
    ClientWindowMove {
        window_id: u32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    ClientSystemCommand {
        window_id: u32,
        command: u32,
    },
    MicConfig {
        sample_rate: u32,
        frames_per_packet: u32,
    },
    WindowPixels {
        window_id: u32,
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
}

pub type Sender = flume::Sender<RdpMessage>;
