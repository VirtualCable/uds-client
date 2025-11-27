#![allow(dead_code)]
use anyhow::Result;
use eframe::egui;

use super::{AppWindow, types::AppState};

impl AppWindow {
    pub fn enter_rdp_connecting(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        self.resize_and_center(ctx, [400.0, 300.0]);
        self.set_app_state(AppState::RdpConnecting);

        Ok(())
    }

    pub fn exit_rdp_connecting(&mut self, _ctx: &eframe::egui::Context) {
        // Any cleanup if necessary
    }

    pub fn update_connection(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Connect to RDP Server");
            ui.label("Comnecting...");
        });
    }
}
