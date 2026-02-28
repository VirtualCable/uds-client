// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use crossbeam::channel::{Receiver, Sender, bounded};

use shared::{log, system::trigger::Trigger};

mod about;
mod asyncthread;

#[macro_use]
mod intl;
mod logo;
mod runner;

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
    if args.len() != 2 || !args[1].starts_with("udssv2://") {
        return None;
    }

    let host_ticket_and_scrambler = &args[1]["udssv2://".len()..];
    match host_ticket_and_scrambler.split_once('/') {
        Some((host, rest)) => match rest.split_once('/') {
            Some((ticket, scrambler)) if ticket.len() == crypt::consts::TICKET_LENGTH => {
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

    js::gui::set_sender(messages_tx.clone());

    // Launch async thread with tokio runtime
    asyncthread::run(messages_tx, stop.clone(), host, ticket, scrambler);

    // Run the GUI, this will block until the GUI is closed
    gui::run_gui(intl::get_catalog().clone(), None, messages_rx, stop.clone()).unwrap();

    // Gui closed, wait for app to finish also
    stop.wait();
}
