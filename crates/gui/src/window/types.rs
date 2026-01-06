// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
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

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::sync::{Arc, RwLock};

use tokio::sync::oneshot;

use super::{client_progress, rdp::connection, rdp::preconnection};

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
    RdpConnecting(preconnection::RdpConnectingState),
    RdpConnected(connection::RdpConnectionState),
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
    ToggleFPS,
    Skip,
}

impl HotKey {
    pub fn from_input(ctx: &eframe::egui::Context) -> Self {
        ctx.input(|input| {
            if is_maximized(input.viewport().maximized.unwrap_or(false)) {
                return Self::ToggleFullScreen; // If maximized state changed, toggle fullscreen
            }

            if !input.modifiers.alt || !input.modifiers.command || input.modifiers.shift {
                return Self::None;
            }

            if input.key_pressed(eframe::egui::Key::Enter) {
                Self::ToggleFullScreen
            } else if input.key_pressed(eframe::egui::Key::F) {
                Self::ToggleFPS
            } else if input.key_released(eframe::egui::Key::Enter)
                || input.key_released(eframe::egui::Key::F)
            {
                Self::Skip
            } else {
                Self::None
            }
        })
    }
}
