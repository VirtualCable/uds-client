// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::{Arc, RwLock};

use tokio::sync::oneshot;

/// Messages the GUI can receive from external code
#[derive(Debug)]
pub enum GuiMessage {
    Close,
    ShowError(String),
    ShowWarning(String),
    ShowYesNo(String, Arc<RwLock<Option<oneshot::Sender<bool>>>>),
    ShowProgress,
    Progress(u8, String),
    ConnectRdp(Box<rdp_ffi::settings::RdpSettings>),
    CloseProgress,
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
