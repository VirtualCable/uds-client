use eframe::egui;
use shared::system::trigger::Trigger;

enum GuiMessage {
    Close,         // Close window
    Error(String), // Error message, and then close
}

struct Launcher {
    progress: f32,
    direction: f32,
    rx: std::sync::mpsc::Receiver<GuiMessage>,
    error: Option<String>,
    stop: Trigger,
}

impl Launcher {
    fn new(stop: Trigger) -> (Self, std::sync::mpsc::Sender<GuiMessage>) {
        let (tx, rx) = std::sync::mpsc::channel();
        (
            Launcher {
                progress: 0.0,
                direction: 0.01,
                rx,
                error: None,
                stop,
            },
            tx,
        )
    }
}

impl eframe::App for Launcher {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process incoming messages
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                GuiMessage::Close => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                GuiMessage::Error(text) => {
                    self.error = Some(text);
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                if let Some(err) = &self.error {
                    ui.label(format!("Error: {}", err));
                    if ui.button("Cerrar").clicked() {
                        std::process::exit(1); // salir de toda la app
                    }
                } else {
                    // UI normal
                    self.progress = (self.progress + self.direction).clamp(0.0, 1.0);
                    if self.progress == 1.0 || self.progress == 0.0 {
                        self.direction = -self.direction;
                    }

                    ui.add(egui::ProgressBar::new(self.progress).animate(true));

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(20.0);

                    if ui.button("Cancel").clicked() {
                        self.stop.set();
                    }
                }
            });
        });
    }

}

fn main() {
    let stop = Trigger::new();
    let (launcher, tx) = Launcher::new(stop.clone());

    std::thread::spawn({
        let stop = stop.clone();
        move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            // Blocking call to async code
            rt.block_on({
                let stop = stop.clone();
                async move {
                    // ... lógica async
                    // tx.send(GuiMessage::Close).ok();
                    stop.async_wait().await;
                    tx.send(GuiMessage::Close).ok();
                }
            });
        }
    });

    // Lanzamos la ventana (bloqueante hasta que se cierre)
    let native_options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "UDS Launcher",
        native_options,
        Box::new(|_cc| Ok(Box::new(launcher))),
    );

    // Aquí llegamos cuando la ventana se cierra
    stop.wait(); // bloquea hasta que la tarea async dispare el notify
}
