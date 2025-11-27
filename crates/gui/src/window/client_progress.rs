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
    progress: Arc<AtomicU16>, // Progress percentage (0-100)
    progress_message: String,
    start: Instant,
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
    pub fn enter_client_progress(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        log::debug!("Switching to client progress window...");
        self.resize_and_center(ctx, [320.0, 280.0]);
        ctx.send_viewport_cmd(egui::ViewportCommand::Title("UDS Launcher - Progress".to_string()));

        self.set_app_state(AppState::ClientProgress(ProgressState {
            progress: Arc::new(AtomicU16::new(0)),
            progress_message: String::new(),
            start: Instant::now(),
        }));
        Ok(())
    }

    pub fn exit_client_progress(&mut self) {
        log::debug!("Exiting client progress window...");
    }

    pub fn update_progress(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
        _state: &ProgressState,
    ) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Progress...");
        });
    }
}
