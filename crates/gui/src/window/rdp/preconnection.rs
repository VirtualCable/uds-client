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
use anyhow::Result;
use eframe::egui;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use rdp::settings::RdpSettings; // crate module, not super :)

use crate::window::{AppWindow, types::AppState};

#[derive(Clone, Debug)]
pub struct RdpConnectingState {
    settings: RdpSettings,
    start: Instant,
    switch_to_fullscreen: Arc<AtomicBool>,
}

impl AppWindow {
    pub fn enter_rdp_preconnection(
        &mut self,
        ctx: &eframe::egui::Context,
        _frame: &mut eframe::Frame,
        settings: RdpSettings,
    ) -> Result<()> {
        // Default size for connecting window if no fullscreen
        // Will be resized later for fullscreen or for fixed size
        // if screen size is fullscreen, start with a simple screen for windowd of 1024x768
        let screen_size = settings.screen_size;
        self.resize_and_center(
            ctx,
            [screen_size.width() as f32, screen_size.height() as f32],
            true,
        );
        self.set_app_state(AppState::RdpConnecting(RdpConnectingState {
            settings,
            start: Instant::now(),
            switch_to_fullscreen: Arc::new(AtomicBool::new(screen_size.is_fullscreen())),
        }));

        Ok(())
    }

    pub fn update_rdp_preconnection(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        state: RdpConnectingState,
    ) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if state.start.elapsed().as_millis() > 100 {
                if state.settings.screen_size.is_fullscreen() {
                    // Get size now that window is created
                    let screen_size = ctx.content_rect().size();
                    self.screen_size = Some((screen_size.x as u32, screen_size.y as u32));
                }
                // Switch to RdpConnected after 1 second, this is only for setting fullscreen etc.
                if let Err(err) = self.enter_rdp_connection(ctx, frame, state.settings.clone()) {
                    self.enter_error(
                        ctx,
                        frame,
                        format!("Failed to connect to RDP server: {}", err),
                    )
                    .ok();
                    return;
                }
                if state.switch_to_fullscreen.load(Ordering::Relaxed) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                    state.switch_to_fullscreen.store(false, Ordering::Relaxed);
                }
                ui.label("Connecting to RDP server...");
            }
        });
    }
}
