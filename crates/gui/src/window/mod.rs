#![allow(dead_code)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use crossbeam::channel::Receiver;
use eframe::egui;

use crate::input;

use shared::{log, system::trigger::Trigger};

mod client_progress;
mod rdp_connected;
mod rdp_connecting;

mod msgw_error;
mod msgw_warning;
mod msgw_yesno;

pub mod types;

const FRAMES_IN_FLIGHT: usize = 128;

pub(super) struct AppWindow {
    pub app_state: types::AppState,
    pub prev_app_state: types::AppState,
    pub texture: egui::TextureHandle, // Logo texture, useful for various windows
    pub processing_events: Arc<AtomicBool>, // Set if we need to process events
    pub events: Receiver<input::RawKey>,
    pub gui_messages_rx: Receiver<types::GuiMessage>,
    pub stop: Trigger, // For stopping any ongoing operations
}

impl AppWindow {
    pub fn new(
        processing_events: Arc<AtomicBool>,
        events: Receiver<input::RawKey>,
        gui_messages_rx: Receiver<types::GuiMessage>,
        stop: Trigger,
        cc: &eframe::CreationContext<'_>,
    ) -> Self {
        processing_events.store(false, Ordering::Relaxed); // Initially not processing events
        let texture = cc.egui_ctx.load_texture(
            "empty",
            crate::logo::load_logo(),
            egui::TextureOptions::LINEAR,
        );
        Self {
            app_state: types::AppState::RdpConnecting,
            prev_app_state: types::AppState::Invisible,
            texture,
            events,
            gui_messages_rx,
            processing_events,
            stop,
        }
    }

    pub fn resize_and_center(&mut self, ctx: &eframe::egui::Context, size: impl Into<egui::Vec2>) {
        let size = size.into();
        let screen_size = ctx.content_rect().size();
        let x_coord = (screen_size.x - size.x) / 2.0;
        let y_coord = (screen_size.y - size.y) / 2.0;
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
            [x_coord, y_coord].into(),
        ));
    }

    pub fn set_app_state(&mut self, new_state: types::AppState) {
        self.processing_events.store(false, Ordering::Relaxed); // Stop processing rdp raw events on event loop
        self.prev_app_state = self.app_state.clone();
        self.app_state = new_state;
    }

    pub fn setup_invisible(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        self.set_app_state(types::AppState::Invisible);

        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        Ok(())
    }
}

impl eframe::App for AppWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(16)); // Approx 60 FPS
        // First, process any incoming GUI messages
        while let Ok(msg) = self.gui_messages_rx.try_recv() {
            match msg {
                types::GuiMessage::Close => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    return;
                }
                types::GuiMessage::ShowError(msg) => {
                    self.setup_error(ctx, msg.clone()).ok();
                }
                types::GuiMessage::ShowWarning(msg) => {
                    self.setup_warning(ctx, msg.clone()).ok();
                }
                types::GuiMessage::ShowYesNo(msg, resp_tx) => {
                    self.setup_yesno(ctx, msg.clone(), resp_tx).ok();
                }
                _ => {
                    log::warn!("Unhandled GUI message: {:?}", msg);
                }
            }
        }

        // States shoud be clonable to work correctly
        // And changes should be reflected on all references
        let mut app_state = self.app_state.clone();
        match &mut app_state {
            types::AppState::RdpConnecting => self.update_connection(ctx, frame),
            types::AppState::RdpConnected(rdp_state) => self.update_rdp_client(ctx, frame, rdp_state),
            types::AppState::ClientProgress(client_state) => self.update_progress(ctx, frame, client_state),
            types::AppState::Invisible => {} // Nothing to do
            types::AppState::YesNo(message, resp_tx) => self.update_yesno(ctx, frame, message, resp_tx),
            types::AppState::Warning(message) => self.update_warning(ctx, frame, message),
            types::AppState::Error(message) => self.update_error(ctx, frame, message),
        }
    }
}
