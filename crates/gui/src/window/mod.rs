#![allow(dead_code)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use crossbeam::channel::Receiver;
use eframe::egui;

use super::{input, types::AppState, types::GuiMessage};
use crate::log;

mod client_progress;
mod rdp_connected;
mod rdp_connecting;

mod msgw_error;
mod msgw_warning;
mod msgw_yesno;

const FRAMES_IN_FLIGHT: usize = 128;

pub enum State {
    Progress(client_progress::ProgressState),
    Rdp(rdp_connected::RdpState),
    None,
}

pub(super) struct AppWindow {
    pub prev_app_state: AppState,
    pub app_state: AppState,
    pub inner_state: State,
    pub processing_events: Arc<AtomicBool>, // Set if we need to process events
    pub events: Receiver<input::RawKey>,
    pub gui_messages_rx: Receiver<GuiMessage>,
}

impl AppWindow {
    pub fn new(
        processing_events: Arc<AtomicBool>,
        events: Receiver<input::RawKey>,
        gui_messages_rx: Receiver<GuiMessage>,
    ) -> Self {
        processing_events.store(false, Ordering::Relaxed); // Initially not processing events
        Self {
            app_state: AppState::RdpConnecting,
            prev_app_state: AppState::RdpConnecting,
            inner_state: State::None,
            events,
            gui_messages_rx,
            processing_events,
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

    pub fn switch_to_invisible(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        self.app_state = AppState::Invisible;
        self.inner_state = State::None;

        self.processing_events.store(false, Ordering::Relaxed); // Stop processing events
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        Ok(())
    }

    pub fn switch_to(&mut self, ctx: &eframe::egui::Context, state: AppState) {
        self.processing_events.store(false, Ordering::Relaxed); // Stop processing events

        let res = match state {
            AppState::RdpConnecting => self.switch_to_rdp_connecting(ctx),
            AppState::RdpConnected => self.switch_to_rdp_connected(ctx),
            AppState::ClientProgress => self.switch_to_client_progress(ctx),
            AppState::Invisible => self.switch_to_invisible(ctx),
            AppState::YesNo => self.switch_to_yesno(ctx),
            AppState::Warning => self.switch_to_warning(ctx),
            AppState::Error => self.switch_to_error(ctx),
        };
        if let Err(e) = res {
            log::error!("Failed to switch GUI state: {}", e);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            log::info!("Switched GUI state to {:?}", state);
            self.prev_app_state = self.app_state;
            self.app_state = state;
        }
    }
}

impl eframe::App for AppWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(16)); // Approx 60 FPS
        match &mut self.app_state {
            AppState::RdpConnecting => self.update_connection(ctx, frame),
            AppState::RdpConnected => self.update_rdp_client(ctx, frame),
            AppState::ClientProgress => self.update_progress(ctx, frame),
            AppState::Invisible => {} // Nothing to do
            AppState::YesNo => self.update_yesno(ctx, frame),
            AppState::Warning => self.update_warning(ctx, frame),
            AppState::Error => self.update_error(ctx, frame),
        }
    }
}
