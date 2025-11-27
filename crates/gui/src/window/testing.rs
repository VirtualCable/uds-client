#![allow(dead_code)]
use anyhow::Result;
use crossbeam::channel::bounded;
use eframe::egui;

use super::{AppWindow, types::AppState};

impl AppWindow {
    pub fn setup_testing(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize([400.0, 300.0].into()));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition([10.0, 10.0].into()));
        self.set_app_state(AppState::RdpConnecting);

        Ok(())
    }

    pub fn update_testing(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Connect to RDP Server");
            ui.label("Enter server details and connect.");
            // Here you can add input fields for server, user, password, etc.
            if ui.button("Connect").clicked() {
                // For demonstration, we use a hardcoded host
                if let Err(e) = self.enter_rdp_connected(ctx) {
                    ui.label(format!("Failed to connect: {}", e));
                }
            }
            if ui.button("Progress").clicked()
                && let Err(e) = self.enter_client_progress(ctx)
            {
                ui.label(format!("Failed to show progress: {}", e));
            }

            if ui.button("Invisible").clicked()
                && let Err(e) = self.enter_invisible(ctx)
            {
                ui.label(format!("Failed to go invisible: {}", e));
            }

            if ui.button("Warning").clicked()
                && let Err(e) = self.enter_warning(ctx, "This is a warning message.".to_string())
            {
                ui.label(format!("Failed to show warning: {}", e));
            }

            if ui.button("Error").clicked()
                && let Err(e) = self.enter_error(ctx, "This is an error message.".to_string())
            {
                ui.label(format!("Failed to show error: {}", e));
            }

            if ui.button("Yes/No").clicked() {
                let (resp_tx, _resp_rx) = bounded::<bool>(1);
                if let Err(e) =
                    self.enter_yesno(ctx, "Do you want to continue?".to_string(), Some(resp_tx))
                {
                    ui.label(format!("Failed to show yes/no dialog: {}", e));
                }
            }
        });
    }
}
