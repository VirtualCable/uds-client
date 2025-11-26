#![allow(dead_code)]
use anyhow::Result;
use eframe::egui;

use super::{AppState, AppWindow};

impl AppWindow {
    pub fn switch_to_rdp_connecting(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize([400.0, 300.0].into()));
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition([10.0, 10.0].into()));

        Ok(())
    }

    pub fn update_connection(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Connect to RDP Server");
            ui.label("Enter server details and connect.");
            // Here you can add input fields for server, user, password, etc.
            if ui.button("Connect").clicked() {
                // For demonstration, we use a hardcoded host
                if let Err(e) = self.switch_to_rdp_connected(ctx) {
                    ui.label(format!("Failed to connect: {}", e));
                }
            }
            if ui.button("Progress").clicked() {
                self.switch_to(ctx, AppState::ClientProgress);
            }

            if ui.button("Invisible").clicked() {
                self.switch_to(ctx, AppState::Invisible);
            }
        });
    }
}
