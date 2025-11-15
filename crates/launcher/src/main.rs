#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;
use shared::system::trigger::Trigger;

mod gui;
mod logo;

fn main() {
    let stop = Trigger::new();
    let (launcher, tx) = gui::Launcher::new(stop.clone());

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
                    for i in 0..=100 {
                        tx.send(gui::GuiMessage::Progress(i as f32 / 100.0))
                            .ok();
                        if stop.async_wait_timeout(std::time::Duration::from_millis(20)).await {
                            break;  // Exit if triggered
                        }
                    }
                    //tx.send(gui::GuiMessage::Close).ok();
                    tx.send(gui::GuiMessage::Error("Simulated error\nlets see how it looks\nhttps://www.udsenterprise.com\nwith several lines\nand more".to_string()))
                        .ok();
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
            Ok(Box::new(launcher))
        }),
    );
    // Gui closed, wait for app to finish also
    stop.wait();
}
