#![allow(dead_code)]
use crossbeam::channel::Sender;

pub enum ProgressMessage {
    Start(String),       // Start progress with message
    Update(f32, String), // Update progress with percentage and message
    Finish,              // Finish progress
}

pub enum GuiMessage {
    Close,                               // Close gui
    Error(String),                       // Error message, and then close
    Warning(String),                     // Warning message, but do not close
    YesNo(String, Option<Sender<bool>>), // Yes/No dialog
    // Progress
    SwitchToClientProgress,
    Progress(ProgressMessage),
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum AppState {
    Invisible,
    ClientProgress,
    RdpConnecting,
    RdpConnected,
    YesNo,
    Error,
    Warning,
}
