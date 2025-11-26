#![allow(dead_code)]
use std::time::Instant;

use anyhow::Result;
use eframe::egui;

use super::{super::types::GuiMessage, AppState, AppWindow, State};

pub struct ProgressState {
    progress: f32,
    progress_message: String,
    // stop: Trigger,  // Will be reintegrated wen on client app
    message: Option<GuiMessage>,
    texture: Option<egui::TextureHandle>,
    start: Instant,
}

impl AppWindow {
    pub fn switch_to_client_progress(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        self.resize_and_center(ctx, [320.0, 280.0]);

        self.app_state = AppState::ClientProgress;
        self.inner_state = State::Progress(ProgressState {
            progress: 0.0,
            progress_message: String::new(),
            // stop: Trigger::new(),
            message: None,
            texture: None,
            start: Instant::now(),
        });
        Ok(())
    }

    pub fn update_progress(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Progress...");
        });
    }
}
