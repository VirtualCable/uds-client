// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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
use anyhow::Result;

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
};
use std::{
    fmt::Debug,
    sync::{Arc, RwLock},
};

use shared::{system::trigger::Trigger};

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
        if let Ok(text) = self.context.read().unwrap().get_text() {
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
