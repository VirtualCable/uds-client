use anyhow::Result;
use crossbeam::channel::Sender;
use eframe::egui;

use shared::log;

use super::{
    AppWindow,
    helper::{calculate_text_height, display_multiline_text},
    types::AppState,
};

impl AppWindow {
    pub fn enter_yesno(
        &mut self,
        ctx: &egui::Context,
        message: String,
        resp_tx: Option<Sender<bool>>,
    ) -> Result<()> {
        let text_height = calculate_text_height(&message, 40, 18.0);
        self.resize_and_center(ctx, [320.0, text_height + 48.0]);
        self.set_app_state(AppState::YesNo(message, resp_tx));
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.gettext("Question")));
        Ok(())
    }

    pub fn update_yesno(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
        message: &str,
        resp_tx: &mut Option<Sender<bool>>,
    ) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_width(300.0);
            ui.horizontal_centered(|ui: &mut egui::Ui| {
                ui.vertical_centered(|ui: &mut egui::Ui| {
                    display_multiline_text(ui, message, self.gettext("Click to open link"));
                });
            });
            egui::TopBottomPanel::bottom("warning_button_panel")
                .show_separator_line(false)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.columns(2, |columns| {
                            columns[0].with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_sized(
                                            [80.0, 30.0],
                                            egui::Button::new(self.gettext("Yes")),
                                        )
                                        .clicked()
                                    {
                                        log::debug!("User clicked Yes");
                                        if let Some(tx) = resp_tx.take() {
                                            let _ = tx.send(true);
                                        }
                                        self.restore_previous_state(ctx);
                                    }
                                },
                            );
                            columns[1].with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_sized(
                                            [80.0, 30.0],
                                            egui::Button::new(self.gettext("No")),
                                        )
                                        .clicked()
                                    {
                                        log::debug!("User clicked No");
                                        if let Some(tx) = resp_tx.take() {
                                            let _ = tx.send(false);
                                        }
                                        self.restore_previous_state(ctx);
                                    }
                                },
                            );
                        });
                    });
                });
        });
    }
}
