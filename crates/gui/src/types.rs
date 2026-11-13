// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::{Arc, RwLock};

use tokio::sync::oneshot;

pub use crate::windows::dialog::{
    DialogButton, DialogButtonResult, DialogField, DialogFieldResult, DialogFieldType,
    DialogResult, DialogSpec,
};

/// Messages the GUI can receive from external code
#[derive(Debug)]
pub enum GuiMessage {
    Close,
    ShowError(String),
    ShowWarning(String),
    ShowYesNo(String, Arc<RwLock<Option<oneshot::Sender<bool>>>>),
    ShowCustomDialog(
        DialogSpec,
        Arc<RwLock<Option<oneshot::Sender<DialogResult>>>>,
    ),
    ShowProgress,
    Progress(u8, String),
    ConnectRdp(Box<rdp::settings::RdpSettings>),
    ConnectUrp(Box<UrpSettings>),
    CloseProgress,
}

/// Channel switches of a URP session (which optional channels the client
/// offers). Defaults mirror the broker's conservative policy: audio,
/// clipboard and storage on, the rest off.
#[derive(Debug, Clone, Copy)]
pub struct UrpRedirections {
    pub audio: bool,
    pub mic: bool,
    pub clipboard: bool,
    pub printing: bool,
    pub webcam: bool,
    pub smartcard: bool,
    pub storage: bool,
}

impl Default for UrpRedirections {
    fn default() -> Self {
        Self {
            audio: true,
            mic: false,
            clipboard: true,
            printing: false,
            webcam: false,
            smartcard: false,
            storage: true,
        }
    }
}

/// Core settings of a UDS Remote Protocol (URP) session, translated from
/// the JS namespace parameters. Free of protocol-crate types so the gui
/// crate does not depend on it.
#[derive(Clone)]
pub struct UrpSettings {
    /// Server host (direct, or the local tunnel endpoint).
    pub server: String,
    /// Server port (default 14433).
    pub port: u16,
    /// Wire ticket (broker ticket string, UTF-8 NUL-padded to 48 bytes).
    pub ticket: [u8; 48],
    /// Session PSK (shared secret of the script's KEM envelope).
    pub key: [u8; 32],
    /// Interactive logon credentials collected from the user when the
    /// broker did not provide them (`has_credentials = false`).
    pub user: Option<String>,
    pub password: Option<String>,
    pub domain: Option<String>,
    /// Broker-fixed remote resolution; (0, 0) = not fixed.
    pub screen_width: u32,
    pub screen_height: u32,
    /// Start in fullscreen.
    pub fullscreen: bool,
    /// Allow stretching the remote image when fullscreen exceeds it.
    pub allow_fullscreen_scaling: bool,
    /// Cap for the auto (monitor) fullscreen resolution.
    pub max_fullscreen_width: u32,
    pub max_fullscreen_height: u32,
    pub redirections: UrpRedirections,
    /// Codec preset preference (`None` = server ladder decides).
    pub preferred_codec: Option<String>,
}

impl std::fmt::Debug for UrpSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Ticket, key and password are secrets: never logged.
        f.debug_struct("UrpSettings")
            .field("server", &self.server)
            .field("port", &self.port)
            .field("ticket", &"<redacted>")
            .field("key", &"<redacted>")
            .field("user", &self.user)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field("domain", &self.domain)
            .field("screen_width", &self.screen_width)
            .field("screen_height", &self.screen_height)
            .field("fullscreen", &self.fullscreen)
            .field("allow_fullscreen_scaling", &self.allow_fullscreen_scaling)
            .field("max_fullscreen_width", &self.max_fullscreen_width)
            .field("max_fullscreen_height", &self.max_fullscreen_height)
            .field("redirections", &self.redirections)
            .field("preferred_codec", &self.preferred_codec)
            .finish()
    }
}

/// Return code from run_gui()
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnCode {
    Exit,
    RestartTestingLauncher,
}

/// Initial state for the GUI
#[derive(Debug, Clone)]
pub enum AppState {
    Test,
    Progress,
}

/// Hotkeys recognized during RDP session
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HotKey {
    #[default]
    None,
    ToggleFullScreen,
    ToggleFPS,
    Skip,
}
