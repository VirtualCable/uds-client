use anyhow::Result;
use eframe::egui;

use super::{
    AppWindow,
    helper::{calculate_text_height, display_multiline_text},
    types::AppState,
};

impl AppWindow {
    pub fn enter_error(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
        message: String,
    ) -> Result<()> {
        let text_height = calculate_text_height(&message, 40);
        self.resize_and_center(ctx, [320.0, text_height + 48.0], true);
        self.set_app_state(AppState::Error(message));
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.gettext("Error")));
        Ok(())
    }

    pub fn update_error(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame, message: &str) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_width(300.0);
            ui.horizontal_centered(|ui: &mut egui::Ui| {
                ui.vertical_centered(|ui: &mut egui::Ui| {
                    display_multiline_text(ui, message, self.gettext("Click to open link"));
                });
            });
            egui::TopBottomPanel::bottom("error_button_panel")
                .show_separator_line(false)
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui: &mut egui::Ui| {
                        ui.vertical_centered(|ui: &mut egui::Ui| {
                            ui.add_space(12.0);
                            if ui
                                .add_sized([80.0, 30.0], egui::Button::new(self.gettext("Ok")))
                                .clicked()
                            {
                                // Set stop
                                self.stop.set();
                            }
                            ui.add_space(12.0);
                        });
                    });
                    ui.add_space(12.0);
                });
        });
    }
}
