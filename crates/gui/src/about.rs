#![allow(dead_code)]
use eframe::egui;
use std::time::Instant;

// About is a simple about dialog, that runs in its own window
// because it will be shown only when requested by main

const ABOUT_TEXT: &[&str] = &[
    "UDS Launcher",
    "Version: 5.0.0",
    "UDS Client Launcher",
    "",
    "Developed by UDS Enterprise",
    "https://www.udsenterprise.com",
    "",
    "This software is provided 'as-is',",
    "without any express or implied warranty.",
    "In no event will the authors be held liable",
    "for any damages arising from the use of this software.",
];

struct About {
    texture: egui::TextureHandle,
    start: Instant,
}

impl About {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let img = super::logo::load_logo();
        let texture = cc.egui_ctx.load_texture("logo", img, egui::TextureOptions::LINEAR);
        About {
            texture,
            start: Instant::now(),
        }
    }
}

impl eframe::App for About {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));

        let elapsed = self.start.elapsed().as_secs_f32();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(30.0);
            ui.horizontal_centered(|ui| {
                ui.vertical_centered(|ui| {
                    ui.set_width(380.0); // width fixed
                    ui.add_sized(
                        [80.0, 80.0],
                        egui::Image::new(&self.texture).rotate(elapsed.sin() / 2.0, [0.5, 0.5].into()),
                    );
                    for line in ABOUT_TEXT {
                        ui.label(*line);
                    }

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(20.0);

                    if ui.add_sized([80.0, 30.0], egui::Button::new("Close")).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });
    }
}

pub fn show_about_window() {
    let icon = super::logo::load_icon();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(true)
            .with_inner_size([420.0, 440.0])
            .with_icon(icon)
            .with_title("About UDS Launcher")
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };
    let _ = eframe::run_native(
        "UDS Launcher",
        native_options,
        Box::new(|cc| {
            // Return the app implementation.
            Ok(Box::new(About::new(cc)))
        }),
    );
}
