use std::sync::{Arc, RwLock};

use anyhow::Result;
use eframe::egui;
use tokio::sync::oneshot;

use rdp::{geom::ScreenSize, settings::RdpSettings};

use super::{AppWindow, client_progress::ProgressState, types::AppState};

impl AppWindow {
    pub fn enter_testing(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        self.resize_and_center(ctx, [400.0, 300.0], true);
        self.set_app_state(AppState::Test);

        Ok(())
    }

    pub fn restore_testing(&mut self, ctx: &eframe::egui::Context) -> Result<()> {
        self.enter_testing(ctx)
    }

    pub fn update_testing(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Test Screen");
            ui.label("Select action.");
            // Here you can add input fields for server, user, password, etc.
            if ui.button("RDP Connecting").clicked() {
                // For demonstration, we use a hardcoded host
                if let Err(e) = self.enter_rdp_preconnection(
                    ctx,
                    RdpSettings {
                        server: "172.27.247.161".to_string(),
                        user: "user".to_string(),
                        password: "temporal".to_string(),
                        screen_size: ScreenSize::Full, // ScreenSize::Fixed(1600, 900),
                        ..RdpSettings::default()
                    },
                ) {
                    ui.label(format!("Failed to start connecting: {}", e));
                }
            }
            if ui.button("RDP Connect").clicked() {
                // For demonstration, we use a hardcoded host
                if let Err(e) = self.enter_rdp_connection(
                    ctx,
                    RdpSettings {
                        server: "172.27.247.161".to_string(),
                        user: "user".to_string(),
                        password: "temporal".to_string(),
                        screen_size: ScreenSize::Full, // ScreenSize::Fixed(1600, 900),
                        ..RdpSettings::default()
                    },
                ) {
                    ui.label(format!("Failed to connect: {}", e));
                }
            }
            if ui.button("Progress").clicked()
                && let Err(e) = self.enter_client_progress(ctx, ProgressState::default())
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
                let (resp_tx, _resp_rx) = oneshot::channel::<bool>();
                if let Err(e) = self.enter_yesno(
                    ctx,
                    "Do you want to continue?".to_string(),
                    Arc::new(RwLock::new(Some(resp_tx))),
                ) {
                    ui.label(format!("Failed to show yes/no dialog: {}", e));
                }
            }
        });
    }
}
