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
use zeroize::Zeroize;

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
    None, // Used on interrupting recv by a timeout

    // Unified RAIL window events
    WindowCreate {
        window_id: u32,
        owner_id: Option<u32>,
        style: Option<u32>,
        ext_style: Option<u32>,
        taskbar_button: Option<bool>,
        title: String,
        show_state: Option<u32>,
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
        show_state: Option<u32>,
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
    // uds-client specific commands
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

    // rdphtml5 specific commands
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

pub type Sender = flume::Sender<RdpMessage>;
pub type Receiver = flume::Receiver<RdpMessage>;
pub type CommandSender = flume::Sender<RdpCommand>;
pub type CommandReceiver = flume::Receiver<RdpCommand>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdp_message_update_rects() {
        let rects = vec![Rect::new(0, 0, 100, 100), Rect::new(50, 50, 50, 50)];
        let msg = RdpMessage::UpdateRects(rects.clone());
        match msg {
            RdpMessage::UpdateRects(r) => assert_eq!(r, rects),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_message_disconnect() {
        let msg = RdpMessage::Disconnect;
        assert!(matches!(msg, RdpMessage::Disconnect));
    }

    #[test]
    fn test_rdp_message_focus_required() {
        let msg = RdpMessage::FocusRequired;
        assert!(matches!(msg, RdpMessage::FocusRequired));
    }

    #[test]
    fn test_rdp_message_error() {
        let msg = RdpMessage::Error("test error".to_string());
        match msg {
            RdpMessage::Error(e) => assert_eq!(e, "test error"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_message_set_cursor_icon() {
        let data = vec![0u8; 64];
        let msg = RdpMessage::SetCursorIcon(data.clone(), 10, 20, 32, 32);
        match msg {
            RdpMessage::SetCursorIcon(d, x, y, w, h) => {
                assert_eq!(d, data);
                assert_eq!(x, 10);
                assert_eq!(y, 20);
                assert_eq!(w, 32);
                assert_eq!(h, 32);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_message_clipboard_data() {
        let msg = RdpMessage::ClipboardData("hello".to_string());
        match msg {
            RdpMessage::ClipboardData(s) => assert_eq!(s, "hello"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_message_none() {
        assert!(matches!(RdpMessage::None, RdpMessage::None));
    }

    #[test]
    fn test_rdp_message_window_create() {
        let msg = RdpMessage::WindowCreate {
            window_id: 42,
            owner_id: Some(0),
            style: Some(0),
            ext_style: Some(0),
            taskbar_button: Some(true),
            title: "Test".to_string(),
            show_state: Some(1),
            is_offscreen: Some(false),
            pos: Some((0, 0)),
            size: Some((100, 100)),
        };
        match msg {
            RdpMessage::WindowCreate {
                window_id,
                title,
                show_state,
                is_offscreen,
                ..
            } => {
                assert_eq!(window_id, 42);
                assert_eq!(title, "Test");
                assert_eq!(show_state, Some(1));
                assert_eq!(is_offscreen, Some(false));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_message_window_update() {
        let msg = RdpMessage::WindowUpdate {
            window_id: 7,
            owner_id: Some(0),
            style: Some(0),
            ext_style: Some(0),
            taskbar_button: Some(true),
            title: "Updated".to_string(),
            show_state: Some(1),
            is_offscreen: Some(false),
            pos: Some((0, 0)),
            size: Some((100, 100)),
        };
        match msg {
            RdpMessage::WindowUpdate {
                window_id,
                title,
                show_state,
                is_offscreen,
                ..
            } => {
                assert_eq!(window_id, 7);
                assert_eq!(title, "Updated");
                assert_eq!(show_state, Some(1));
                assert_eq!(is_offscreen, Some(false));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_message_window_delete() {
        let msg = RdpMessage::WindowDelete(99);
        match msg {
            RdpMessage::WindowDelete(id) => assert_eq!(id, 99),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_message_client_window_move() {
        let msg = RdpMessage::ClientWindowMove {
            window_id: 1,
            x: 100,
            y: 200,
            width: 800,
            height: 600,
        };
        match msg {
            RdpMessage::ClientWindowMove {
                window_id,
                x,
                y,
                width,
                height,
            } => {
                assert_eq!(window_id, 1);
                assert_eq!(x, 100);
                assert_eq!(y, 200);
                assert_eq!(width, 800);
                assert_eq!(height, 600);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_message_client_system_command() {
        let msg = RdpMessage::ClientSystemCommand {
            window_id: 5,
            command: 0xF060,
        };
        match msg {
            RdpMessage::ClientSystemCommand { window_id, command } => {
                assert_eq!(window_id, 5);
                assert_eq!(command, 0xF060);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_message_mic_config() {
        let msg = RdpMessage::MicConfig {
            sample_rate: 44100,
            frames_per_packet: 256,
        };
        match msg {
            RdpMessage::MicConfig {
                sample_rate,
                frames_per_packet,
            } => {
                assert_eq!(sample_rate, 44100);
                assert_eq!(frames_per_packet, 256);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_message_debug() {
        let msg = RdpMessage::Disconnect;
        let debug = format!("{:?}", msg);
        assert!(debug.starts_with("Disconnect") || debug.starts_with("RdpMessage"));
    }

    #[test]
    fn test_rdp_message_clone() {
        let msg = RdpMessage::FocusRequired;
        let cloned = msg.clone();
        assert!(matches!(cloned, RdpMessage::FocusRequired));
    }

    #[test]
    fn test_rdp_command_keyboard() {
        let cmd = RdpCommand::Keyboard {
            is_down: true,
            repeat: false,
            scancode: 0x1E,
        };
        match cmd {
            RdpCommand::Keyboard {
                is_down,
                repeat,
                scancode,
            } => {
                assert!(is_down);
                assert!(!repeat);
                assert_eq!(scancode, 0x1E);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_command_mouse() {
        let cmd = RdpCommand::Mouse {
            flags: 0x8001,
            x: 100,
            y: 200,
        };
        match cmd {
            RdpCommand::Mouse { flags, x, y } => {
                assert_eq!(flags, 0x8001);
                assert_eq!(x, 100);
                assert_eq!(y, 200);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_command_resize() {
        let cmd = RdpCommand::Resize {
            width: 1920,
            height: 1080,
        };
        match cmd {
            RdpCommand::Resize { width, height } => {
                assert_eq!(width, 1920);
                assert_eq!(height, 1080);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_rdp_command_focus_in() {
        assert!(matches!(RdpCommand::FocusIn, RdpCommand::FocusIn));
    }

    #[test]
    fn test_rdp_command_debug() {
        let cmd = RdpCommand::FocusIn;
        let debug = format!("{:?}", cmd);
        assert!(debug.starts_with("FocusIn") || debug.starts_with("RdpCommand"));
    }

    #[test]
    fn test_rdp_command_clone() {
        let cmd = RdpCommand::FocusIn;
        let cloned = cmd.clone();
        assert!(matches!(cloned, RdpCommand::FocusIn));
    }

    #[test]
    fn test_sender_type_alias() {
        let (tx, _rx): (Sender, flume::Receiver<RdpMessage>) = flume::unbounded();
        tx.send(RdpMessage::None).unwrap();
    }

    #[test]
    fn test_command_channel() {
        let (cmd_tx, cmd_rx): (CommandSender, CommandReceiver) = flume::unbounded();
        cmd_tx.send(RdpCommand::FocusIn).unwrap();
        assert!(matches!(cmd_rx.recv().unwrap(), RdpCommand::FocusIn));
    }

    // New tests for uds-client commands
    #[test]
    fn test_rdp_command_input_keyboard() {
        let event = InputEvent::Keyboard {
            scancode: 0x1E,
            pressed: true,
            repeat: false,
        };
        let cmd = RdpCommand::Input(event);
        if let RdpCommand::Input(InputEvent::Keyboard {
            scancode,
            pressed,
            repeat,
        }) = cmd
        {
            assert_eq!(scancode, 0x1E);
            assert!(pressed);
            assert!(!repeat);
        } else {
            panic!("Expected Input Keyboard");
        }
    }

    #[test]
    fn test_rdp_command_viewport_move() {
        let cmd = RdpCommand::ViewportMove {
            window_id: 10,
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        };
        if let RdpCommand::ViewportMove {
            window_id,
            left,
            top,
            right,
            bottom,
        } = cmd
        {
            assert_eq!(window_id, 10);
            assert_eq!(left, 0);
            assert_eq!(top, 0);
            assert_eq!(right, 800);
            assert_eq!(bottom, 600);
        } else {
            panic!("Expected ViewportMove");
        }
    }

    #[test]
    fn test_rdp_command_launch_rail_app() {
        let cmd = RdpCommand::LaunchRailApp {
            app: "calc.exe".to_string(),
            args: "".to_string(),
            dir: "".to_string(),
        };
        if let RdpCommand::LaunchRailApp { app, args, dir } = cmd {
            assert_eq!(app, "calc.exe");
            assert_eq!(args, "");
            assert_eq!(dir, "");
        } else {
            panic!("Expected LaunchRailApp");
        }
    }

    #[test]
    fn test_rdp_command_close() {
        assert!(matches!(RdpCommand::Close, RdpCommand::Close));
    }

    #[test]
    fn test_input_event_variants() {
        let ev_mouse = InputEvent::Mouse {
            flags: 1,
            x: 10,
            y: 20,
        };
        if let InputEvent::Mouse { flags, x, y } = ev_mouse.clone() {
            assert_eq!(flags, 1);
            assert_eq!(x, 10);
            assert_eq!(y, 20);
        } else {
            panic!("Expected Mouse");
        }

        let ev_ext = InputEvent::ExtendedMouse {
            flags: 2,
            x: 30,
            y: 40,
        };
        if let InputEvent::ExtendedMouse { flags, x, y } = ev_ext.clone() {
            assert_eq!(flags, 2);
            assert_eq!(x, 30);
            assert_eq!(y, 40);
        } else {
            panic!("Expected ExtendedMouse");
        }

        let ev_uni = InputEvent::Unicode { code: 65 };
        if let InputEvent::Unicode { code } = ev_uni.clone() {
            assert_eq!(code, 65);
        } else {
            panic!("Expected Unicode");
        }
    }

    #[test]
    fn test_input_event_zeroize() {
        let mut ev = InputEvent::Keyboard {
            scancode: 0x1E,
            pressed: true,
            repeat: false,
        };
        ev.zeroize();
        if let InputEvent::Keyboard {
            scancode,
            pressed,
            repeat,
        } = ev
        {
            assert_eq!(scancode, 0);
            assert!(!pressed);
            assert!(!repeat);
        } else {
            panic!("Expected Keyboard");
        }
    }

    #[test]
    fn test_rdp_message_debug_no_vectors() {
        let msg_pixels = RdpMessage::WindowPixels {
            window_id: 1,
            width: 100,
            height: 200,
            data: vec![1, 2, 3, 4],
        };
        let debug_str = format!("{:?}", msg_pixels);
        assert!(debug_str.contains("WindowPixels"));
        assert!(debug_str.contains("window_id: 1"));
        assert!(debug_str.contains("width: 100"));
        assert!(debug_str.contains("height: 200"));
        assert!(!debug_str.contains("data:"));

        let msg_icon = RdpMessage::WindowIcon {
            window_id: 2,
            rgba: vec![255; 16],
            width: 4,
            height: 4,
        };
        let debug_str_icon = format!("{:?}", msg_icon);
        assert!(debug_str_icon.contains("WindowIcon"));
        assert!(!debug_str_icon.contains("rgba:"));
    }

    #[test]
    fn test_rdp_message_untested_variants() {
        let msg_resize = RdpMessage::DesktopResize(1024, 768);
        if let RdpMessage::DesktopResize(w, h) = msg_resize {
            assert_eq!(w, 1024);
            assert_eq!(h, 768);
        } else {
            panic!("Expected DesktopResize");
        }
    }
}
