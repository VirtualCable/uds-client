// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

#[derive(Debug, Clone)]
pub enum InputEvent {
    Keyboard { scancode: u16, pressed: bool },
    Mouse { flags: u16, x: u16, y: u16 },
    ExtendedMouse { flags: u16, x: u16, y: u16 },
    Unicode { code: u16 },
}

#[derive(Debug, Clone)]
pub enum RdpCommand {
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
}

pub type Sender = flume::Sender<RdpCommand>;
pub type Receiver = flume::Receiver<RdpCommand>;
