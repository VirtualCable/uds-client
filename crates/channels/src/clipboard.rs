// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
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
