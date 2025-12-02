use crossbeam::channel::Sender;
use std::{
    sync::{LazyLock, Mutex},
};

use shared::{
    log,
};

pub use gui::window::types::GuiMessage;

// We need a Sender<GuiMessage> to be able to use any gui related functionality in JS
// So ensure to register it
static SENDER: LazyLock<Mutex<Option<Sender<GuiMessage>>>> = LazyLock::new(|| Mutex::new(None));

/// Set the Sender<GuiMessage> to be used by the JS gui module
/// This should be called once during initialization of launcher
pub fn set_sender(sender: Sender<GuiMessage>) {
    log::debug!("Setting GUI message sender for JS gui module");
    let mut guard = SENDER.lock().unwrap();
    *guard = Some(sender);
}

/// Send a GuiMessage to the GUI thread if the sender is set
/// If no sender is set, the message is ignored
pub fn send_message(msg: GuiMessage) {
    log::debug!("Sending GUI message from JS gui module");
    let guard = SENDER.lock().unwrap();
    if let Some(tx) = &*guard {
        tx.send(msg).ok();
    } else {
        log::warn!("No GUI message sender set, ignoring message");
    }
}
