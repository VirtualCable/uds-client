use eframe::egui;

#[derive(Clone)]
pub struct Fps {
    pub last_instant: std::time::Instant,
    pub frames_instants: Vec<f32>,
    pub enabled: bool,
}

impl Fps {
    pub fn new() -> Self {
        Self {
            last_instant: std::time::Instant::now(),
            frames_instants: Vec::with_capacity(128),
            enabled: false,
        }
    }

    pub fn record_frame(&mut self) {
        let delta = self.last_instant.elapsed().as_secs_f32();
        self.last_instant = std::time::Instant::now();

        self.frames_instants.push(delta);
        if self.frames_instants.len() > 128 {
            self.frames_instants.remove(0);
        }
    }

    pub fn average_fps(&self) -> f32 {
        let total_time: f32 = self.frames_instants.iter().sum();
        if total_time > 0.0 {
            self.frames_instants.len() as f32 / total_time
        } else {
            0.0
        }
    }

    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    pub fn show(&self, ctx: &egui::Context) {
        if !self.enabled {
            return;
        }
        egui::Area::new("fps_info".into())
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-64.0, 0.0)) // Centered at top
            .order(egui::Order::Foreground) // Above all layers
            .constrain(true) // Keep within screen bounds
            .show(ctx, |ui| {
                // Frame with margins so it does not occupy the entire width
                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: 64,
                        top: 8,
                        right: 16,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!("FPS: {:.1}", self.average_fps()))
                                .color(egui::Color32::BLACK),
                        );
                        // ui.label("Other info here...");
                    });
            });
    }
}

impl Default for Fps {
    fn default() -> Self {
        Self::new()
    }
}
