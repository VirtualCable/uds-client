use std::sync::{Arc, RwLock};

use tokio::sync::oneshot;

use super::{client_progress, rdp_connected};
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
    RdpConnecting,
    RdpConnected(rdp_connected::RdpState),
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
            if input.key_pressed(eframe::egui::Key::Enter) && input.modifiers.alt || input.modifiers.command
            {
                Self::ToggleFullScreen
            } else {
                Self::None
            }
        })
    }
}
