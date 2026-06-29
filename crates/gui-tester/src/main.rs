// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use flume::{Receiver, Sender, bounded};

use shared::{log, system::trigger::Trigger};

fn main() {
    let fake_catalog = gettext::Catalog::empty(); // Empty catalog for now
    log::setup_logging("debug", log::LogType::Test);
    rdp::wlog::setup_freerdp_logger(rdp::wlog::WLogLevel::Info);
    // Enable TRACE for smartcard-specific tags
    rdp::wlog::set_wlog_level(
        Some("com.freerdp.channels.smartcard.vgids"),
        rdp::wlog::WLogLevel::Trace,
    );
    rdp::wlog::set_wlog_level(
        Some("com.freerdp.utils.smartcard.ops"),
        rdp::wlog::WLogLevel::Trace,
    );

    let (_messages_tx, messages_rx): (
        Sender<gui::types::GuiMessage>,
        Receiver<gui::types::GuiMessage>,
    ) = bounded(32);

    let stop_trigger = Trigger::new();

    gui::run_gui(
        fake_catalog,
        gui::types::AppState::Test,
        messages_rx,
        stop_trigger.clone(),
        None,
    )
    .unwrap();

    stop_trigger.trigger();
}
