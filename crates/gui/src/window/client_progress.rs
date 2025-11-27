#![allow(dead_code)]
use std::{fmt, sync::Arc, time::Instant};

use anyhow::Result;
use eframe::egui;

use shared::log;

use super::{AppWindow, types::AppState};

#[derive(Clone)]
pub struct ProgressState {
    progress: Arc<f32>,
    progress_message: String,
    start: Instant,
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
    pub fn setup_client_progress(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        log::debug!("Switching to client progress window...");
        self.resize_and_center(ctx, [320.0, 280.0]);

        self.set_app_state(AppState::ClientProgress(ProgressState {
            progress: Arc::new(0.0),
            progress_message: String::new(),
            start: Instant::now(),
        }));
        Ok(())
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
