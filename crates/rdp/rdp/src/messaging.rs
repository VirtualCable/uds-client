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

use flume;

use crate::geom::Rect;

#[derive(Clone)]
#[allow(dead_code)]
pub enum RdpMessage {
    UpdateRects(Vec<Rect>),
    Disconnect,
    FocusRequired,
    Error(String),
    SetCursorIcon(Vec<u8>, u32, u32, u32, u32), // x, y, (of pointer "pointer") width, height
    ClipboardData(String),
    None, // Used on interrrupting recv by a timeout

    // Unified RAIL window events
    WindowCreate {
        window_id: u32,
        owner_id: u32,
        style: u32,
        ext_style: u32,
        taskbar_button: bool,
        title: String,
        show_state: u32,
        is_offscreen: bool,
        pos: (i32, i32),
        size: (u32, u32),
    },
    WindowUpdate {
        window_id: u32,
        owner_id: u32,
        style: u32,
        ext_style: u32,
        taskbar_button: bool,
        title: String,
        show_state: u32,
        is_offscreen: bool,
        pos: (i32, i32),
        size: (u32, u32),
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
    WebcamConfig {
        format: u32,
        width: u32,
        height: u32,
        fps: u32,
    },
    StartWebcamStream,
    StopWebcamStream,

    // Mode B specific messages (unified)
    WindowPixels {
        window_id: u32,
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
    WindowIcon {
        window_id: u32,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    DesktopResize(u32, u32),
}

// Debug without printing huge binary/vector fields
impl core::fmt::Debug for RdpMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RdpMessage::UpdateRects(rects) => f
                .debug_struct("UpdateRects")
                .field("count", &rects.len())
                .finish(),
            RdpMessage::Disconnect => f.debug_struct("Disconnect").finish(),
            RdpMessage::FocusRequired => f.debug_struct("FocusRequired").finish(),
            RdpMessage::Error(s) => f.debug_tuple("Error").field(s).finish(),
            RdpMessage::SetCursorIcon(_, x, y, w, h) => f
                .debug_tuple("SetCursorIcon")
                .field(x)
                .field(y)
                .field(w)
                .field(h)
                .finish(),
            RdpMessage::ClipboardData(s) => f.debug_tuple("ClipboardData").field(s).finish(),
            RdpMessage::None => f.debug_struct("None").finish(),
            RdpMessage::WindowCreate {
                window_id, title, ..
            } => f
                .debug_struct("WindowCreate")
                .field("window_id", window_id)
                .field("title", title)
                .finish(),
            RdpMessage::WindowUpdate {
                window_id, title, ..
            } => f
                .debug_struct("WindowUpdate")
                .field("window_id", window_id)
                .field("title", title)
                .finish(),
            RdpMessage::WindowDelete(window_id) => {
                f.debug_tuple("WindowDelete").field(window_id).finish()
            }
            RdpMessage::ClientWindowMove { window_id, .. } => f
                .debug_struct("ClientWindowMove")
                .field("window_id", window_id)
                .finish(),
            RdpMessage::ClientSystemCommand { window_id, command } => f
                .debug_struct("ClientSystemCommand")
                .field("window_id", window_id)
                .field("command", command)
                .finish(),
            RdpMessage::MicConfig {
                sample_rate,
                frames_per_packet,
            } => f
                .debug_struct("MicConfig")
                .field("sample_rate", sample_rate)
                .field("frames_per_packet", frames_per_packet)
                .finish(),
            RdpMessage::WebcamConfig {
                format,
                width,
                height,
                fps,
            } => f
                .debug_struct("WebcamConfig")
                .field("format", format)
                .field("width", width)
                .field("height", height)
                .field("fps", fps)
                .finish(),
            RdpMessage::StartWebcamStream => f.debug_struct("StartWebcamStream").finish(),
            RdpMessage::StopWebcamStream => f.debug_struct("StopWebcamStream").finish(),
            RdpMessage::WindowPixels {
                window_id,
                width,
                height,
                ..
            } => f
                .debug_struct("WindowPixels")
                .field("window_id", window_id)
                .field("width", width)
                .field("height", height)
                .finish(),
            RdpMessage::WindowIcon {
                window_id,
                width,
                height,
                ..
            } => f
                .debug_struct("WindowIcon")
                .field("window_id", window_id)
                .field("width", width)
                .field("height", height)
                .finish(),
            RdpMessage::DesktopResize(width, height) => f
                .debug_struct("DesktopResize")
                .field("width", width)
                .field("height", height)
                .finish(),
        }
    }
}

pub type Sender = flume::Sender<RdpMessage>;
pub type Receiver = flume::Receiver<RdpMessage>;
