#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use crossbeam::channel::{Receiver, Sender, bounded};

use shared::{log, system::trigger::Trigger};

fn main() {
    let fake_catalog = gettext::Catalog::empty(); // Empty catalog for now
    log::setup_logging("trace", log::LogType::Tests);
    let (_messages_tx, messages_rx): (
        Sender<gui::window::types::GuiMessage>,
        Receiver<gui::window::types::GuiMessage>,
    ) = bounded(32);

    gui::run_gui(
        fake_catalog,
        Some(gui::window::types::AppState::Test),
        messages_rx,
        Trigger::new(),
    )
    .unwrap();
}
