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
