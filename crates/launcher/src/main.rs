#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use crossbeam::channel::{Receiver, Sender, bounded};

use shared::{consts, log, system::trigger::Trigger};

mod about;
mod asyncthread;

#[macro_use]
mod intl;
mod logo;
mod runner;

fn collect_arguments() -> Option<(String, String, String)> {
    let args: Vec<String> = std::env::args().collect();
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
    #[cfg(debug_assertions)]
    log::setup_logging("debug", log::LogType::Launcher);
    #[cfg(not(debug_assertions))]
    log::setup_logging("info", log::LogType::Launcher);
    // Setup tls, with default secure ciphers
    shared::tls::init_tls(None);
    let (host, ticket, scrambler) = collect_arguments().unwrap_or_else(|| {
        // Show about window if no valid arguments
        about::show_about_window();
        std::process::exit(0);
    });

    log::debug!(
        "Host: {}, Ticket: {}, Scrambler: {}",
        host,
        ticket,
        scrambler
    );

    let stop = Trigger::new();
    let (messages_tx, messages_rx): (
        Sender<gui::window::types::GuiMessage>,
        Receiver<gui::window::types::GuiMessage>,
    ) = bounded(32);

    // Launch async thread with tokio runtime
    asyncthread::run(messages_tx.clone(), stop.clone(), host, ticket, scrambler);

    js::gui::set_sender(messages_tx.clone());

    gui::run_gui(intl::get_catalog().clone(), None, messages_rx, stop.clone()).unwrap();

    // Gui closed, wait for app to finish also
    stop.wait();
}
