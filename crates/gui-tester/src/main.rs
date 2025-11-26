#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use shared::log;

fn main() {
    log::setup_logging("trace", log::LogType::Tests);
    gui::run_gui().unwrap();
}
