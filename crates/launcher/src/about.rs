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
use eframe::egui;
use std::time::Instant;

const ABOUT_TEXT: &[&str] = &[
    "UDS Launcher",
    "Version: 5.0.0",
    "UDS Client Launcher",
    "",
    "Developed by UDS Enterprise",
    "https://www.udsenterprise.com",
    "",
    "This software is provided 'as-is',",
    "without any express or implied warranty.",
    "In no event will the authors be held liable",
    "for any damages arising from the use of this software.",
];

struct About {
    texture: Option<egui::TextureHandle>,
    start: Instant,
}

impl About {
    pub fn new() -> Self {
        About {
            texture: None,
            start: Instant::now(),
        }
    }
}

impl Default for About {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for About {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Load texture the first time
        if self.texture.is_none() {
            let img = crate::logo::load_logo();
            self.texture = Some(ctx.load_texture("logo", img, egui::TextureOptions::LINEAR));
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(50));

        let elapsed = self.start.elapsed().as_secs_f32();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(30.0);
            ui.horizontal_centered(|ui| {
                ui.vertical_centered(|ui| {
                    ui.set_width(380.0); // width fixed
                    if let Some(tex) = &self.texture {
                        ui.add_sized(
                            [80.0, 80.0],
                            egui::Image::new(tex).rotate(elapsed.sin() / 2.0, [0.5, 0.5].into()),
                        );
                    }
                    for line in ABOUT_TEXT {
                        ui.label(*line);
                    }

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(20.0);

                    if ui.add_sized([80.0, 30.0], egui::Button::new("Close")).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });
    }
}

pub fn show_about_window() {
    let about = About::new();
    let icon = crate::logo::load_icon();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(true)
            .with_inner_size([420.0, 440.0])
            .with_icon(icon)
            .with_title("About UDS Launcher")
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };
    let _ = eframe::run_native(
        "UDS Launcher",
        native_options,
        Box::new(|_cc| {
            // Return the app implementation.
            Ok(Box::new(about))
        }),
    );
}