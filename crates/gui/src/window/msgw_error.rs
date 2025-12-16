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
use anyhow::Result;
use eframe::egui;

use super::{
    AppWindow,
    helper::{calculate_text_height, display_multiline_text},
    types::AppState,
};

impl AppWindow {
    pub fn enter_error(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
        message: String,
    ) -> Result<()> {
        let text_height = calculate_text_height(&message, 40);
        self.resize_and_center(ctx, [320.0, text_height + 48.0], true);
        self.set_app_state(AppState::Error(message));
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.gettext("Error")));
        Ok(())
    }

    pub fn update_error(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame, message: &str) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_width(300.0);
            ui.horizontal_centered(|ui: &mut egui::Ui| {
                ui.vertical_centered(|ui: &mut egui::Ui| {
                    display_multiline_text(ui, message, self.gettext("Click to open link"));
                });
            });
            egui::TopBottomPanel::bottom("error_button_panel")
                .show_separator_line(false)
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui: &mut egui::Ui| {
                        ui.vertical_centered(|ui: &mut egui::Ui| {
                            ui.add_space(12.0);
                            if ui
                                .add_sized([80.0, 30.0], egui::Button::new(self.gettext("Ok")))
                                .clicked()
                            {
                                // Set stop
                                self.stop.set();
                            }
                            ui.add_space(12.0);
                        });
                    });
                    ui.add_space(12.0);
                });
        });
    }
}
