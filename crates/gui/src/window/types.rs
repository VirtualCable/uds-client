use std::sync::{Arc, RwLock};

use tokio::sync::oneshot;

use super::{client_progress, rdp_connection, rdp_preconnection};

static WAS_MAXIMIZED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn is_maximized(current_maximized: bool) -> bool {
    let previous = WAS_MAXIMIZED.load(std::sync::atomic::Ordering::Relaxed);
    WAS_MAXIMIZED.store(current_maximized, std::sync::atomic::Ordering::Relaxed);
    previous != current_maximized && current_maximized
}

#[derive(Debug)]
pub enum GuiMessage {
    Close,                                                         // Close gui
    Hide,                // Hide window, but keep app running
    ShowError(String),   // Error message, and then close
    ShowWarning(String), // Warning message
    ShowYesNo(String, Arc<RwLock<Option<oneshot::Sender<bool>>>>), // Yes/No dialog
    // Progress
    ShowProgress,
    Progress(f32, String), // progress percentage (0.0-100.0), message
    ConnectRdp(rdp::settings::RdpSettings), // Connect RDP with given settings
}

#[derive(Debug, Clone, Default)]
pub enum AppState {
    #[default]
    Invisible, // Default state, window is hidden
    Test, // Testing window
    ClientProgress(client_progress::ProgressState),
    // use this to set fullscreen prior to connection if needed
    // and anything else
    RdpConnecting(rdp_preconnection::RdpConnectingState),
    RdpConnected(rdp_connection::RdpConnectionState),
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
            if (input.key_pressed(eframe::egui::Key::Enter) && input.modifiers.alt)
                || is_maximized(input.viewport().maximized.unwrap_or(false))
            {
                // Send restore so maximixed is toggled off and return toggle fullscreen
                Self::ToggleFullScreen
            } else {
                Self::None
            }
        })
    }
}
