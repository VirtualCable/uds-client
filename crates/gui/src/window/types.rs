#![allow(dead_code)]
use crossbeam::channel::Sender;

use super::{client_progress, rdp_connected};

#[derive(Debug)]
pub enum ProgressMessage {
    Start(String),       // Start progress with message
    Update(f32, String), // Update progress with percentage and message
    Finish,              // Finish progress
}

#[derive(Debug)]
pub enum GuiMessage {
    Close,                                   // Close gui
    ShowError(String),                       // Error message, and then close
    ShowWarning(String),                     // Warning message, but do not close
    ShowYesNo(String, Option<Sender<bool>>), // Yes/No dialog
    // Progress
    SwitchToClientProgress,
    Progress(ProgressMessage),
}

#[derive(Debug, Clone, Default)]
pub enum AppState {
    #[default]
    Invisible, // Default state, window is hidden
    Test,      // Testing window
    ClientProgress(client_progress::ProgressState),
    RdpConnecting,
    RdpConnected(rdp_connected::RdpState),
    YesNo(String, Option<Sender<bool>>),
    Error(String),
    Warning(String),
}
