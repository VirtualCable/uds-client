// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::{Arc, Mutex, RwLock};

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
};
use rdp::integrations::ClipboardCallback;
use rdp::integrations::ClipboardIntegration;
use shared::system::trigger::Trigger;

#[derive(Clone)]
pub struct ClipboardHandle {
    context: Arc<RwLock<ClipboardContext>>,
    stop_trigger: Arc<Mutex<Option<Trigger>>>,
}

impl std::fmt::Debug for ClipboardHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClipboardHandle")
            .field("stop_trigger", &self.stop_trigger)
            .finish()
    }
}

impl ClipboardHandle {
    pub fn new() -> Self {
        let context = ClipboardContext::new().unwrap();
        Self {
            context: Arc::new(RwLock::new(context)),
            stop_trigger: Arc::new(Mutex::new(None)),
        }
    }
}

pub struct ClipboardController {
    callback: Arc<dyn ClipboardCallback>,
    context: Arc<RwLock<ClipboardContext>>,
}

impl ClipboardHandler for ClipboardController {
    fn on_clipboard_change(&mut self) {
        if let Ok(text) = self.context.read().unwrap().get_text() {
            self.callback.send_text_is_available(&text);
        }
    }
}

impl ClipboardIntegration for ClipboardHandle {
    fn start(&self, callback: Arc<dyn ClipboardCallback>) {
        self.stop();

        let trigger = Trigger::new();
        *self.stop_trigger.lock().unwrap() = Some(trigger.clone());

        let context = Arc::clone(&self.context);
        let mut watcher_context = ClipboardWatcherContext::new().unwrap();
        let manager = ClipboardController { callback, context };
        let watcher_shutdown = watcher_context.add_handler(manager).get_shutdown_channel();

        std::thread::spawn(move || {
            watcher_context.start_watch();
        });

        let stop_trigger = trigger.clone();
        std::thread::spawn(move || {
            stop_trigger.wait();
            watcher_shutdown.stop();
        });
    }

    fn stop(&self) {
        if let Some(trigger) = self.stop_trigger.lock().unwrap().take() {
            trigger.trigger();
        }
    }

    fn set_text(&self, text: &str) -> anyhow::Result<()> {
        let context = self.context.write().unwrap();
        context
            .set_text(text.to_string())
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn get_text(&self) -> anyhow::Result<String> {
        let context = self.context.read().unwrap();
        context.get_text().map_err(|e| anyhow::anyhow!(e))
    }
}

impl Default for ClipboardHandle {
    fn default() -> Self {
        Self::new()
    }
}
