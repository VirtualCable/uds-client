use eframe::egui;
use shared::system::trigger::Trigger;
use std::time::Instant;
use tokio::sync::oneshot;

use shared::{log, utils::split_lines};

use crate::tr;

#[allow(dead_code)]
pub enum GuiMessage {
    Close,                                        // Close window
    Error(String),                                // Error message, and then close
    Warning(String),                              // Warning message, but do not close
    YesNo(String, Option<oneshot::Sender<bool>>), // Yes/No dialog
    Progress(f32),                                // Update progress bar
}

pub struct Progress {
    progress: f32,
    rx: std::sync::mpsc::Receiver<GuiMessage>,
    stop: Trigger,
    message: Option<GuiMessage>,
    texture: Option<egui::TextureHandle>,
    start: Instant,
}

impl Progress {
    pub fn new(stop: Trigger) -> (Self, std::sync::mpsc::Sender<GuiMessage>) {
        let (tx, rx) = std::sync::mpsc::channel();
        (
            Progress {
                progress: 0.0,
                rx,
                stop,
                message: None,
                texture: None,
                start: Instant::now(),
            },
            tx,
        )
    }
}

impl eframe::App for Progress {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.texture.is_none() {
            let img = crate::logo::load_logo();
            self.texture = Some(ctx.load_texture("logo", img, egui::TextureOptions::LINEAR));
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(50));

        // Process incoming messages
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                GuiMessage::Close => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                GuiMessage::Error(text) => {
                    self.message = Some(GuiMessage::Error(text));
                }
                GuiMessage::Warning(text) => {
                    self.message = Some(GuiMessage::Warning(text));
                }
                GuiMessage::YesNo(text, sender) => {
                    self.message = Some(GuiMessage::YesNo(text, sender));
                }
                GuiMessage::Progress(p) => {
                    self.progress = p;
                }
            }
        }

        let elapsed = self.start.elapsed().as_secs_f32();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(30.0);
            ui.horizontal_centered(|ui| {
                ui.vertical_centered(|ui| {
                    ui.set_width(200.0); // width fixed
                    if let Some(tex) = &self.texture {
                        ui.add_sized(
                            [80.0, 80.0],
                            egui::Image::new(tex).rotate(elapsed.sin() / 2.0, [0.5, 0.5].into()),
                        );
                    }
                    ui.add(
                        egui::ProgressBar::new(self.progress)
                            .desired_height(24.0)
                            .animate(false)
                            .show_percentage(),
                    );

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(20.0);

                    if ui.button("Cancel").clicked() {
                        self.stop.set();
                    }
                });
            });
        });

        match &mut self.message {
            Some(GuiMessage::YesNo(text, reply)) => {
                if messagebox(ctx, tr!("Confirm"), text, reply) {
                    self.message = None; // diálogo cerrado
                }
            }
            Some(GuiMessage::Error(text)) => {
                if messagebox(ctx, tr!("Error"), text, &mut None) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Some(GuiMessage::Warning(text)) => {
                if messagebox(ctx, tr!("Warning"), text, &mut None) {
                    self.message = None;
                }
            }
            _ => {}
        }
    }
}

fn messagebox(
    ctx: &egui::Context,
    title: &str,
    text: &str,
    reply: &mut Option<oneshot::Sender<bool>>,
) -> bool {
    let mut close: bool = false;
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .auto_sized()
        .fade_in(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_width(280.0);
            ui.add_space(10.0);
            ui.horizontal_centered(|ui: &mut egui::Ui| {
                ui.vertical_centered(|ui| {
                    // Split the text by newlines, and append each line separately
                    for line in split_lines(text, 40) {
                        if line.starts_with("http") {
                            // get label after |
                            let (label, link) = if let Some(pos) = line.find('|') {
                                (&line[pos + 1..], &line[..pos])
                            } else {
                                (line, line)
                            };
                            if ui
                                .hyperlink_to(label, link)
                                .on_hover_text(tr!("Click to open in browser"))
                                .clicked()
                            {
                                if let Err(e) = open::that(line) {
                                    log::error!("Failed to open link {}: {}", line, e);
                                }
                            } else {
                                // Because clippy wants to collapse this block
                                // and then the meaning is lost
                                // because we WANT to execute hyperling_to even if not clicked
                                // and not show the label if not clicked.. stupid clippy :)
                            }
                        } else {
                            ui.label(line);
                        }
                    }
                    ui.add_space(14.0);
                    if reply.is_some() {
                        ui.horizontal(|ui| {
                            ui.columns(2, |columns| {
                                columns[0].with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button(tr!("Yes")).clicked() {
                                            // extraemos el sender y lo consumimos
                                            if let Some(tx) = reply.take() {
                                                let _ = tx.send(true);
                                            }
                                            close = true;
                                        }
                                    },
                                );
                                columns[1].with_layout(
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        if ui.button(tr!("No")).clicked() {
                                            if let Some(tx) = reply.take() {
                                                let _ = tx.send(false);
                                            }
                                            close = true;
                                        }
                                    },
                                );
                            });
                        });
                    } else if ui.button(tr!("Ok")).clicked() {
                        close = true;
                    }
                });
            });
        });
    close
}
