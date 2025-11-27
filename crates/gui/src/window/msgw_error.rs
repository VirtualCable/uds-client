#![allow(dead_code)]
use std::time::Instant;

use anyhow::Result;
use eframe::egui;

use super::{
    AppWindow,
    types::{AppState, GuiMessage},
};

pub struct ProgressState {
    progress: f32,
    progress_message: String,
    // stop: Trigger,  // Will be reintegrated wen on client app
    message: Option<GuiMessage>,
    texture: Option<egui::TextureHandle>,
    start: Instant,
}

impl AppWindow {
    pub fn setup_error(&mut self, ctx: &eframe::egui::Context, message: String) -> Result<()> {
        self.resize_and_center(ctx, [320.0, 280.0]);
        self.set_app_state(AppState::Error(message));
        Ok(())
    }

    pub fn update_error(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame, message: &str) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(message);
        });
    }
}
