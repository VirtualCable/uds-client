#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use shared::log;

fn main() {
    let fake_catalog = gettext::Catalog::empty(); // Empty catalog for now
    log::setup_logging("trace", log::LogType::Tests);
    gui::run_gui(fake_catalog, Some(gui::window::types::AppState::Test)).unwrap();
}
