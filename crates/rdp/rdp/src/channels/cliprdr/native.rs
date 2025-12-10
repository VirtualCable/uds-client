use anyhow::Result;

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
};
use std::{
    fmt::Debug,
    sync::{Arc, RwLock},
};

use shared::{log, system::trigger::Trigger};

use super::RdpClipboard;

#[derive(Clone)]
pub struct ClipboardNative {
    context: Arc<RwLock<ClipboardContext>>,
    stop: Trigger,
    rdp_clipboard: RdpClipboard,
}

// Implement debug with excluding non-debuggable fields
impl Debug for ClipboardNative {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipboardNative")
            .field("stop", &self.stop)
            .field("context", &"ClipboardContext")
            .field("rdp_clipboard", &self.rdp_clipboard)
            .finish()
    }
}

impl ClipboardNative {
    pub fn stop(&mut self) {
        self.stop.set();
    }

    pub fn set_text(&self, text: &str) -> Result<()> {
        let context = self.context.write().unwrap();
        context
            .set_text(text.to_string())
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub fn get_text(&self) -> Result<String> {
        let context = self.context.read().unwrap();
        context.get_text().map_err(|e| anyhow::anyhow!(e))
    }

    pub fn clipboard_changed(&self) {
        log::debug!("Clipboard change detected in native clipboard");
        if let Ok(text) = self.context.read().unwrap().get_text() {
            log::debug!("New clipboard text: {}", text);
            // Store on RDP clipboard
            self.rdp_clipboard.send_text_is_available(&text);
        }
    }
}

pub struct ClipboardController {
    native: Arc<RwLock<ClipboardNative>>,
}

impl ClipboardController {
    pub fn new(native: Arc<RwLock<ClipboardNative>>) -> Self {
        Self { native }
    }
}

impl ClipboardHandler for ClipboardController {
    fn on_clipboard_change(&mut self) {
        // Here we can handle clipboard changes and send data to RDP server
        self.native.read().unwrap().clipboard_changed();
    }
}

impl ClipboardNative {
    pub fn new(rdp_clipboard: RdpClipboard) -> Option<Arc<RwLock<Self>>> {
        if let Ok(context) = ClipboardContext::new() {
            let native = Arc::new(RwLock::new(ClipboardNative {
                context: Arc::new(RwLock::new(context)),
                stop: Trigger::new(),
                rdp_clipboard,
            }));
            let manager = ClipboardController::new(native.clone());
            if let Ok(mut watcher_context) = ClipboardWatcherContext::new() {
                let watcher_shutdown = watcher_context.add_handler(manager).get_shutdown_channel();

                std::thread::spawn(move || {
                    watcher_context.start_watch();
                });

                // Stopper will wait for trigger and then shutdown the watcher
                std::thread::spawn({
                    let stop = native.read().unwrap().stop.clone();
                    move || {
                        stop.wait();
                        watcher_shutdown.stop();
                    }
                });

                Some(native)
            } else {
                None
            }
        } else {
            None
        }
    }
}
