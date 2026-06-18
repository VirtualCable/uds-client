// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use flume::{Receiver, Sender, bounded};

use shared::{log, system::trigger::Trigger};

mod asyncthread;

#[macro_use]
mod intl;
mod runner;

fn parse_udssv2_url(raw: &str) -> Option<(String, String, String)> {
    // Expects format: udssv2://host/ticket/scrambler
    let payload = raw.strip_prefix("udssv2://")?;
    let (host, rest) = payload.split_once('/')?;
    let (ticket, scrambler) = rest.split_once('/')?;
    if ticket.len() != crypt::consts::TICKET_LENGTH {
        return None;
    }
    Some((host.to_string(), ticket.to_string(), scrambler.to_string()))
}

fn collect_arguments() -> Option<(String, String, String)> {
    let args: Vec<String> = std::env::args().collect();

    // For debugging purposes, allow setting args via env variable
    #[cfg(debug_assertions)]
    let args: Vec<String> = if let Ok(debug_args) = std::env::var("UDS_DEBUG_ARGS") {
        ["program".to_string(), debug_args].to_vec()
    } else {
        args
    };

    // Should have only 1 argument, "udssv2://host/ticket/scrambler"
    if args.len() != 2 {
        return None;
    }
    parse_udssv2_url(&args[1])
}

fn main() {
    #[cfg(debug_assertions)]
    {
        log::setup_logging("debug", log::LogType::Launcher);
        rdp::wlog::setup_freerdp_logger(rdp::wlog::WLogLevel::Debug);
    }
    #[cfg(not(debug_assertions))]
    {
        log::setup_logging("info", log::LogType::Launcher);
        rdp::wlog::setup_freerdp_logger(rdp::wlog::WLogLevel::Error);
    }

    // Setup tls, with default secure ciphers
    shared::tls::init_tls(None);
    let (host, ticket, scrambler) = collect_arguments().unwrap_or_else(|| {
        // Show about window if no valid arguments
        gui::windows::about::show_about_window();
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
        Sender<gui::types::GuiMessage>,
        Receiver<gui::types::GuiMessage>,
    ) = bounded(32);

    js::gui::set_sender(messages_tx.clone());

    // Launch async thread with tokio runtime
    asyncthread::run(messages_tx, stop.clone(), host, ticket, scrambler);

    // Read app data, which may contain overrides for proxy and ssl settings, and fps limit
    let app_data = shared::appdata::AppData::load();

    // Run the GUI, this will block until the GUI is closed
    gui::run_gui(
        intl::get_catalog().clone(),
        gui::types::AppState::Progress,
        messages_rx,
        stop.clone(),
        app_data.fps_limit,
    )
    .unwrap();

    // Gui closed, wait for app to finish also
    stop.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_ticket() -> String {
        "A".repeat(crypt::consts::TICKET_LENGTH)
    }

    #[test]
    fn valid_url() {
        let url = format!(
            "udssv2://myhost.example.com/{}/scrambler123",
            valid_ticket()
        );
        let result = parse_udssv2_url(&url);
        assert!(result.is_some());
        let (host, ticket, scrambler) = result.unwrap();
        assert_eq!(host, "myhost.example.com");
        assert_eq!(ticket.len(), crypt::consts::TICKET_LENGTH);
        assert_eq!(scrambler, "scrambler123");
    }

    #[test]
    fn no_prefix() {
        assert!(parse_udssv2_url("https://host/ticket/scrambler").is_none());
    }

    #[test]
    fn missing_scrambler() {
        let url = format!("udssv2://host/{}", valid_ticket());
        assert!(parse_udssv2_url(&url).is_none());
    }

    #[test]
    fn ticket_wrong_length() {
        let url = "udssv2://host/short/scrambler";
        assert!(parse_udssv2_url(url).is_none());
    }

    #[test]
    fn extra_segments() {
        let url = format!("udssv2://host/{}/scrambler/extra", valid_ticket());
        // split_once only splits at the first /, so extra segments become part of scrambler
        let result = parse_udssv2_url(&url);
        assert!(result.is_some());
    }

    #[test]
    fn host_only() {
        assert!(parse_udssv2_url("udssv2://host").is_none());
    }
}
