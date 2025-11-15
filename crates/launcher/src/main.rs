#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;
use shared::system::trigger::Trigger;

mod gui;
mod logo;
mod runner;

fn main() {
    // Should have only 1 argument, "udssv2://[ticket]/[scrambler]"
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        // Show about window if no arguments
        gui::about::show_about_window();
        return;
    }

    let stop = Trigger::new();
    let (progress, tx) = gui::progress::Progress::new(stop.clone());

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
                    runner::run(tx.clone(), stop.clone()).await;
                    tx.send(gui::progress::GuiMessage::Close).ok();
                    stop.set();
                }
            });
        }
    });

    let icon = logo::load_icon();

    // Window configuration
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_inner_size([320.0, 240.0])
            .with_icon(icon)
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };

    let _ = eframe::run_native(
        "UDS Launcher",
        native_options,
        Box::new(|_cc| {
            // Return the app implementation.
            Ok(Box::new(progress))
        }),
    );
    // Gui closed, wait for app to finish also
    stop.wait();
}
