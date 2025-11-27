use anyhow::Result;
use eframe::egui;

use super::{AppWindow, types::AppState, helper::{display_multiline_text, calculate_text_height}};

impl AppWindow {
    pub fn enter_error(&mut self, ctx: &egui::Context, message: String) -> Result<()> {
        // Calculate aprox vertical size
        let text_height = calculate_text_height(&message, 40, 18.0);
        self.resize_and_center(ctx, [320.0, text_height + 48.0]);
        self.set_app_state(AppState::Error(message));
        Ok(())
    }

    pub fn exit_error(&mut self, _ctx: &eframe::egui::Context) {
        // Any cleanup if necessary
    }

    pub fn update_error(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame, message: &str) {
        egui::CentralPanel::default().show(ctx, |ui| {
            display_multiline_text(ui, message, self.gettext("Click to open link"));
        });
    }
}
