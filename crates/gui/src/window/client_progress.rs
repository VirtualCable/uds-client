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
#![allow(dead_code)]
use std::{
    fmt,
    sync::{Arc, atomic::AtomicU16},
    time::Instant,
};

use anyhow::Result;
use eframe::egui;

use shared::log;

use super::{AppWindow, types::AppState};

#[derive(Clone)]
pub struct ProgressState {
    pub progress: Arc<AtomicU16>, // Progress percentage (0-100)
    pub progress_message: String,
    pub start: Instant,
}

impl Default for ProgressState {
    fn default() -> Self {
        Self {
            progress: Arc::new(AtomicU16::new(0)),
            progress_message: String::new(),
            start: Instant::now(),
        }
    }
}

impl fmt::Debug for ProgressState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProgressState")
            .field("progress", &self.progress)
            .field("progress_message", &self.progress_message)
            .field("start", &self.start)
            .finish()
    }
}

impl AppWindow {
    pub fn enter_client_progress(
        &mut self,
        ctx: &eframe::egui::Context,
        _frame: &mut eframe::Frame,
        state: ProgressState,
    ) -> Result<()> {
        log::debug!("Switching to client progress window...");
        self.resize_and_center(ctx, [320.0, 220.0], false);

        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
            "UDS Launcher - Progress".to_string(),
        ));

        self.set_app_state(AppState::ClientProgress(state));

        Ok(())
    }

    pub fn restore_client_progress(
        &mut self,
        ctx: &eframe::egui::Context,
        frame: &mut eframe::Frame,
        state: ProgressState,
    ) -> Result<()> {
        self.enter_client_progress(ctx, frame, state)
    }

    pub fn update_progress(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
        state: ProgressState,
    ) {
        let elapsed = state.start.elapsed().as_secs_f32();
        let progress = state.progress.load(std::sync::atomic::Ordering::Relaxed) as f32;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(30.0);
            ui.horizontal_centered(|ui| {
                ui.vertical_centered(|ui| {
                    ui.set_width(200.0); // width fixed
                    ui.add_sized(
                        [80.0, 80.0],
                        egui::Image::new(&self.texture)
                            .rotate(elapsed.sin() / 2.0, [0.5, 0.5].into()),
                    );
                    ui.add(
                        egui::ProgressBar::new(progress)
                            .desired_height(24.0)
                            .animate(false)
                            .show_percentage(),
                    );

                    ui.add_space(12.0);

                    ui.label(&state.progress_message);

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(20.0);

                    if ui.add_sized([80.0, 30.0], egui::Button::new(self.gettext("Cancel"))).clicked() {
                        self.stop.set();  // main update will handle this
                    }
                });
            });
        });
    }
}
