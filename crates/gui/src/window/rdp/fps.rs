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

#[derive(Clone)]
pub struct Fps {
    pub last_instant: std::time::Instant,
    pub frames_instants: Vec<f32>,
    pub enabled: bool,
}

impl Fps {
    pub fn new() -> Self {
        Self {
            last_instant: std::time::Instant::now(),
            frames_instants: Vec::with_capacity(128),
            enabled: false,
        }
    }

    pub fn record_frame(&mut self) {
        let delta = self.last_instant.elapsed().as_secs_f32();
        self.last_instant = std::time::Instant::now();

        self.frames_instants.push(delta);
        if self.frames_instants.len() > 128 {
            self.frames_instants.remove(0);
        }
    }

    pub fn average_fps(&self) -> f32 {
        let total_time: f32 = self.frames_instants.iter().sum();
        if total_time > 0.0 {
            self.frames_instants.len() as f32 / total_time
        } else {
            0.0
        }
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn show(&self, ctx: &egui::Context) {
        if !self.enabled {
            return;
        }
        egui::Area::new("fps_info".into())
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-64.0, 0.0)) // Centered at top
            .order(egui::Order::Foreground) // Above all layers
            .constrain(true) // Keep within screen bounds
            .show(ctx, |ui| {
                // Frame with margins so it does not occupy the entire width
                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: 64,
                        top: 8,
                        right: 16,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!("FPS: {:.1}", self.average_fps()))
                                .color(egui::Color32::BLACK),
                        );
                        // ui.label("Other info here...");
                    });
            });
    }
}

impl Default for Fps {
    fn default() -> Self {
        Self::new()
    }
}
