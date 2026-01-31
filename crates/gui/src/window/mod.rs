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
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use crossbeam::channel::Receiver;
use eframe::egui;

use shared::{log, system::trigger::Trigger};

mod client_progress;
mod rdp;

mod msgw_error;
mod msgw_warning;
mod msgw_yesno;
mod testing;

pub mod types;

mod helper;

const FRAMES_IN_FLIGHT: usize = 128;

pub(super) struct AppWindow {
    pub app_state: types::AppState,
    pub prev_app_state: types::AppState,
    pub texture: egui::TextureHandle, // Logo texture, useful for various windows
    pub processing_events: Arc<AtomicBool>, // Set if we need to process wininit events (keyboard events right now)
    pub events: Receiver<crate::RawKey>,
    pub gui_messages_rx: Receiver<types::GuiMessage>,
    pub stop: Trigger,                   // For stopping any ongoing operations
    pub screen_size: Option<(u32, u32)>, // Cached screen size
    pub catalog: gettext::Catalog,       // For translations
}

impl AppWindow {
    pub fn new(
        processing_events: Arc<AtomicBool>,
        events: Receiver<crate::RawKey>,
        gui_messages_rx: Receiver<types::GuiMessage>,
        stop: Trigger,
        catalog: gettext::Catalog,
        initial_state: Option<types::AppState>,
        cc: &eframe::CreationContext<'_>,
    ) -> Self {
        processing_events.store(false, Ordering::Relaxed); // Initially not processing events
        let texture = cc.egui_ctx.load_texture(
            "empty",
            crate::logo::load_logo(),
            egui::TextureOptions::LINEAR,
        );
        Self {
            app_state: initial_state.unwrap_or_default(),
            prev_app_state: types::AppState::default(),
            texture,
            events,
            gui_messages_rx,
            processing_events,
            stop,
            screen_size: None,
            catalog,
        }
    }

    pub fn gettext(&self, msgid: &str) -> String {
        self.catalog.gettext(msgid).to_string()
    }

    pub fn exit(&mut self, ctx: &eframe::egui::Context) {
        log::debug!("Exiting application...");
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        self.stop.trigger();
    }

    pub fn resize_and_center(
        &mut self,
        ctx: &eframe::egui::Context,
        size: impl Into<egui::Vec2>,
        decorations: bool,
    ) {
        let size = size.into() + [0.0, 48.0].into(); // Add some extra space for title bar
        let screen_size = self.screen_size.unwrap_or((1920, 1080));
        let x_coord = (screen_size.0 as f32 - size.x) / 2.0;
        let y_coord = (screen_size.1 as f32 - size.y) / 2.0;

        // Set window size and position
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(decorations));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
            [x_coord, y_coord].into(),
        ));
    }

    pub fn set_app_state(&mut self, new_state: types::AppState) {
        self.processing_events.store(false, Ordering::Relaxed); // Stop processing rdp raw events on event loop
        // Only testing and client_progress states can go back
        self.prev_app_state = if matches!(
            self.app_state,
            types::AppState::Test | types::AppState::ClientProgress(_)
        ) {
            self.app_state.clone()
        } else {
            types::AppState::default()
        };
        self.app_state = new_state;
    }

    pub fn restore_previous_state(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.processing_events.store(false, Ordering::Relaxed); // Stop processing rdp raw events on event loop
        self.app_state = self.prev_app_state.clone();
        self.prev_app_state = types::AppState::default();
        // Call restore if necessary, that is, for testing and client_progress states
        // Other states do not need restoration
        match &self.app_state {
            types::AppState::Test => self.restore_testing(ctx, frame).ok(),
            types::AppState::ClientProgress(state) => {
                self.restore_client_progress(ctx, frame, state.clone()).ok()
            }
            _ => None,
        };
    }

    pub fn enter_invisible(
        &mut self,
        ctx: &eframe::egui::Context,
        _frame: &mut eframe::Frame,
    ) -> Result<()> {
        self.set_app_state(types::AppState::Invisible);

        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        Ok(())
    }
}

impl eframe::App for AppWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // If stop has been triggered, close the window
        if self.stop.is_triggered() {
            log::debug!("Stop triggered, closing window.");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let frame_start = std::time::Instant::now();
        // First, process any incoming GUI messages
        while let Ok(msg) = self.gui_messages_rx.try_recv() {
            match msg {
                types::GuiMessage::Close => {
                    log::debug!("Received close message, closing window.");
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                types::GuiMessage::Hide => {
                    log::debug!("Received hide message, hiding window.");
                    self.enter_invisible(ctx, frame).ok();
                }
                types::GuiMessage::ShowError(msg) => {
                    log::debug!("Received show error message: {}", msg);
                    self.enter_error(ctx, frame, msg.clone()).ok();
                }
                types::GuiMessage::ShowWarning(msg) => {
                    log::debug!("Received show warning message: {}", msg);
                    self.enter_warning(ctx, frame, msg.clone()).ok();
                }
                types::GuiMessage::ShowYesNo(msg, resp_tx) => {
                    log::debug!("Received show yes/no message: {}", msg);
                    self.enter_yesno(ctx, frame, msg.clone(), resp_tx).ok();
                }
                types::GuiMessage::ShowProgress => {
                    log::debug!("Switching to client progress window...");
                    self.enter_client_progress(
                        ctx,
                        frame,
                        client_progress::ProgressState::default(),
                    )
                    .ok();
                }
                types::GuiMessage::Progress(percentage, message) => {
                    log::debug!("Received progress update: {}% - {}", percentage, message);
                    if let types::AppState::ClientProgress(state) = &mut self.app_state {
                        state.progress.store(
                            (percentage.clamp(0.0, 1.0) * 100.0) as u16,
                            Ordering::Relaxed,
                        );
                        state.progress_message = message;
                    }
                }
                types::GuiMessage::ConnectRdp(settings) => {
                    log::debug!("Received RDP connect message: {:?}", settings);
                    self.enter_rdp_preconnection(ctx, frame, settings).ok();
                }
            }
        }

        // States should be clonable to work correctly
        // And changes should be reflected on all references
        let app_state = self.app_state.clone();
        match app_state {
            types::AppState::RdpConnecting(rdp_state) => {
                self.update_rdp_preconnection(ctx, frame, rdp_state)
            }
            types::AppState::RdpConnected(rdp_state) => {
                self.update_rdp_connection(ctx, frame, rdp_state)
            }
            types::AppState::ClientProgress(client_state) => {
                self.update_progress(ctx, frame, client_state)
            }
            types::AppState::Invisible => {} // Nothing to do
            types::AppState::YesNo(message, resp_tx) => {
                self.update_yesno(ctx, frame, &message, resp_tx)
            }
            types::AppState::Warning(message) => self.update_warning(ctx, frame, &message),
            types::AppState::Error(message) => self.update_error(ctx, frame, &message),
            types::AppState::Test => self.update_testing(ctx, frame),
        }
        let frame_duration = frame_start.elapsed();
        // ctx.request_repaint(); // Repaint asap
        let remaining = std::time::Duration::from_millis(16).saturating_sub(frame_duration);
        ctx.request_repaint_after(remaining); // Aim for ~60 FPS
    }
}
