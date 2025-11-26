#![allow(dead_code)]
use std::{time::Instant};

use anyhow::Result;
use eframe::egui;

use super::{super::types::GuiMessage, AppWindow};

pub struct ProgressState {
    progress: f32,
    progress_message: String,
    // stop: Trigger,  // Will be reintegrated wen on client app
    message: Option<GuiMessage>,
    texture: Option<egui::TextureHandle>,
    start: Instant,
}

impl AppWindow {
    pub fn switch_to_warning(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        self.resize_and_center(ctx, [320.0, 280.0]);

        Ok(())
    }

    pub fn update_warning(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("warning...");
        });
    }
}
