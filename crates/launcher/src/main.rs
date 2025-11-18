#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use eframe::egui;
use shared::system::trigger::Trigger;

use shared::{consts, log};

mod appdata;
mod asyncthread;
mod gui;

#[macro_use]
mod intl;
mod logo;
mod runner;

fn collect_arguments() -> Option<(String, String, String)> {
    // TODO: Use real args
    let args: Vec<String> = std::env::args().collect();
    // let args = [
    //     "program",
    //     "udssv2://172.27.0.1:8443/4l8oo8ptz8k2phbsbvbc3gjsgg4y27d2qimvpn6gvikx4eca/isxN53Woj2rK4B6WfDgCy5K6heF3hCe8",
    // ]; // Some test args for reference
    log::debug!("Command line arguments: {:?}", args);
    // Should have only 1 argument, "udssv2://host/ticket/scrambler"
    if args.len() != 2 || !args[1].starts_with("udssv2://") {
        return None;
    }

    let host_ticket_and_scrambler = &args[1]["udssv2://".len()..];
    match host_ticket_and_scrambler.split_once('/') {
        Some((host, rest)) => match rest.split_once('/') {
            Some((ticket, scrambler)) if ticket.len() == consts::TICKET_LENGTH => {
                Some((host.to_string(), ticket.to_string(), scrambler.to_string()))
            }
            _ => None,
        },
        _ => None,
    }
}

fn main() {
    log::setup_logging("info", log::LogType::Launcher);
    // Setup tls, with default secure ciphers
    shared::tls::init_tls(None);
    let (host, ticket, scrambler) = collect_arguments().unwrap_or_else(|| {
        // Show about window if no valid arguments
        gui::about::show_about_window();
        std::process::exit(0);
    });

    log::debug!(
        "Host: {}, Ticket: {}, Scrambler: {}",
        host,
        ticket,
        scrambler
    );

    let stop = Trigger::new();
    let (progress, tx) = gui::progress::Progress::new(stop.clone());

    // Launch async thread with tokio runtime
    asyncthread::run(tx, stop.clone(), host, ticket, scrambler);

    let icon = logo::load_icon();

    // Window configuration
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_inner_size([320.0, 280.0])
            .with_app_id("UDSLauncher")
            .with_icon(icon)
            .with_resizable(false),
        centered: true,
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "UDS Launcher",
        native_options,
        Box::new(|_cc| {
            // Return the app implementation.
            Ok(Box::new(progress))
        }),
    ) {
        eprintln!("Error starting gui: {}", e);
        log::error!("Error starting gui: {}", e);
    }
    
    // Gui closed, wait for app to finish also
    stop.wait();
}
