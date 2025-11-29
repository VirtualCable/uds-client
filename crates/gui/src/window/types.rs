use std::sync::{Arc, RwLock};

use tokio::sync::oneshot;

use super::{client_progress, rdp_connected, rdp_connecting};


#[derive(Debug)]
pub enum GuiMessage {
    Close,                                                         // Close gui
    Hide,                // Hide window, but keep app running
    ShowError(String),   // Error message, and then close
    ShowWarning(String), // Warning message
    ShowYesNo(String, Arc<RwLock<Option<oneshot::Sender<bool>>>>), // Yes/No dialog
    // Progress
    SwitchToClientProgress,
    Progress(f32, String), // progress percentage (0.0-100.0), message
}

#[derive(Debug, Clone, Default)]
pub enum AppState {
    #[default]
    Invisible, // Default state, window is hidden
    Test, // Testing window
    ClientProgress(client_progress::ProgressState),
    // use this to set fullscreen prior to connection if needed
    // and anything else
    RdpConnecting(rdp_connecting::RdpConnectingState),  
    RdpConnected(rdp_connected::RdpConnectionState),
    // This will be consumed once response is sent and only once
    YesNo(String, Arc<RwLock<Option<oneshot::Sender<bool>>>>),
    Error(String),
    Warning(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HotKey {
    #[default]
    None,
    ToggleFullScreen,
}

impl HotKey {
    pub fn from_input(ctx: &eframe::egui::Context) -> Self {
        ctx.input(|input| {
            if input.key_pressed(eframe::egui::Key::Enter) && input.modifiers.alt
            {
                Self::ToggleFullScreen
            } else {
                Self::None
            }
        })
    }
}
