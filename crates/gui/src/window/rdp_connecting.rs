#![allow(dead_code)]
use anyhow::Result;
use eframe::egui;

use super::{AppWindow, types::AppState};

impl AppWindow {
    pub fn setup_rdp_connecting(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize([400.0, 300.0].into()));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition([10.0, 10.0].into()));
        self.set_app_state(AppState::RdpConnecting);

        Ok(())
    }

    pub fn update_connection(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Connect to RDP Server");
            ui.label("Enter server details and connect.");
            // Here you can add input fields for server, user, password, etc.
            if ui.button("Connect").clicked() {
                // For demonstration, we use a hardcoded host
                if let Err(e) = self.setup_rdp_connected(ctx) {
                    ui.label(format!("Failed to connect: {}", e));
                }
            }
            if ui.button("Progress").clicked()
                && let Err(e) = self.setup_client_progress(ctx)
            {
                ui.label(format!("Failed to show progress: {}", e));
            }

            if ui.button("Invisible").clicked()
                && let Err(e) = self.setup_invisible(ctx)
            {
                ui.label(format!("Failed to go invisible: {}", e));
            }
        });
    }
}
