use eframe::egui;
use shared::system::trigger::Trigger;
use std::time::Instant;

use shared::utils::split_lines;

pub enum GuiMessage {
    Close,         // Close window
    Error(String), // Error message, and then close
    Progress(f32), // Update progress bar
}

pub struct Progress {
    progress: f32,
    rx: std::sync::mpsc::Receiver<GuiMessage>,
    stop: Trigger,
    error: Option<String>,
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
                error: None,
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
                    self.error = Some(text);
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

        if let Some(err) = &self.error {
            messagebox(ctx, "Error", err);
        }
    }
}

fn messagebox(ctx: &egui::Context, title: &str, text: &str) {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .auto_sized()
        .fade_in(true)
        .anchor(egui::Align2::CENTER_CENTER, [5.0, 5.0])
        .show(ctx, |ui| {
            ui.set_width(320.0);
            ui.add_space(10.0);
            ui.horizontal_centered(|ui: &mut egui::Ui| {
                ui.vertical_centered(|ui| {
                    // Split the text by newlines, and append each line separately
                    for line in split_lines(text, 40) {
                        if line.starts_with("http") {
                            if ui
                                .hyperlink_to(line, line)
                                .on_hover_text("Click to open in browser")
                                .clicked()
                            {
                                if let Err(e) = open::that(line) {
                                    eprintln!("Failed to open link {}: {}", line, e);
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
                    if ui.button("Close").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });
}
