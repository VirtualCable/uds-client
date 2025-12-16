// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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

use anyhow::Result;
use eframe::egui;
use tokio::sync::oneshot;

use rdp::{geom::ScreenSize, settings::RdpSettings};

use super::{AppWindow, client_progress::ProgressState, types::AppState};

impl AppWindow {
    pub fn enter_testing(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) -> Result<()> {
        self.resize_and_center(ctx, [400.0, 300.0], true);
        self.set_app_state(AppState::Test);

        Ok(())
    }

    pub fn restore_testing(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) -> Result<()> {
        self.enter_testing(ctx, frame)
    }

    pub fn update_testing(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Test Screen");
            ui.label("Select action.");
            // Here you can add input fields for server, user, password, etc.
            if ui.button("RDP Connecting").clicked() {
                // For demonstration, we use a hardcoded host
                if let Err(e) = self.enter_rdp_preconnection(
                    ctx,
                    frame,
                    RdpSettings {
                        server: "172.27.247.161".to_string(),
                        user: "user".to_string(),
                        password: "temporal".to_string(),
                        screen_size: ScreenSize::Full, // ScreenSize::Fixed(1600, 900),
                        ..RdpSettings::default()
                    },
                ) {
                    ui.label(format!("Failed to start connecting: {}", e));
                }
            }
            if ui.button("RDP Connect").clicked() {
                // For demonstration, we use a hardcoded host
                if let Err(e) = self.enter_rdp_connection(
                    ctx,
                    frame,
                    RdpSettings {
                        server: "172.27.247.161".to_string(),
                        user: "user".to_string(),
                        password: "temporal".to_string(),
                        screen_size: ScreenSize::Full, // ScreenSize::Fixed(1600, 900),
                        ..RdpSettings::default()
                    },
                ) {
                    ui.label(format!("Failed to connect: {}", e));
                }
            }
            if ui.button("Progress").clicked()
                && let Err(e) = self.enter_client_progress(ctx, frame, ProgressState::default())
            {
                ui.label(format!("Failed to show progress: {}", e));
            }

            if ui.button("Invisible").clicked()
                && let Err(e) = self.enter_invisible(ctx, frame)
            {
                ui.label(format!("Failed to go invisible: {}", e));
            }

            if ui.button("Warning").clicked()
                && let Err(e) = self.enter_warning(ctx, frame, "This is a warning message.".to_string())
            {
                ui.label(format!("Failed to show warning: {}", e));
            }

            if ui.button("Error").clicked()
                && let Err(e) = self.enter_error(ctx, frame, "This is an error message.".to_string())
            {
                ui.label(format!("Failed to show error: {}", e));
            }

            if ui.button("Yes/No").clicked() {
                let (resp_tx, _resp_rx) = oneshot::channel::<bool>();
                if let Err(e) = self.enter_yesno(
                    ctx,
                    frame,
                    "Do you want to continue?".to_string(),
                    Arc::new(RwLock::new(Some(resp_tx))),
                ) {
                    ui.label(format!("Failed to show yes/no dialog: {}", e));
                }
            }
        });
    }
}
