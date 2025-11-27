use anyhow::Result;
use crossbeam::channel::Sender;
use eframe::egui;

use super::{AppWindow, types::AppState};
impl AppWindow {
    pub fn enter_yesno(
        &mut self,
        ctx: &egui::Context,
        message: String,
        resp_tx: Option<Sender<bool>>,
    ) -> Result<()> {
        self.resize_and_center(ctx, [320.0, 280.0]);
        self.set_app_state(AppState::YesNo(message, resp_tx));
        Ok(())
    }

    pub fn exit_yesno(&mut self, _ctx: &eframe::egui::Context) {
        // Any cleanup if necessary
    }

    pub fn update_yesno(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
        message: &str,
        _resp_tx: &mut Option<Sender<bool>>,
    ) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(message);
        });
    }
}
