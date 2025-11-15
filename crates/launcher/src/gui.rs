use eframe::egui;
use shared::system::trigger::Trigger;
use std::time::Instant;

pub enum GuiMessage {
    Close,         // Close window
    Error(String), // Error message, and then close
    Progress(f32), // Update progress bar
}

pub struct Launcher {
    progress: f32,
    rx: std::sync::mpsc::Receiver<GuiMessage>,
    error: Option<String>,
    stop: Trigger,

    texture: Option<egui::TextureHandle>,
    start: Instant,
}

impl Launcher {
    pub fn new(stop: Trigger) -> (Self, std::sync::mpsc::Sender<GuiMessage>) {
        let (tx, rx) = std::sync::mpsc::channel();
        (
            Launcher {
                progress: 0.0,
                rx,
                error: None,
                stop,
                texture: None,
                start: Instant::now(),
            },
            tx,
        )
    }
}

impl eframe::App for Launcher {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // cargar textura la primera vez
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
    }
}
