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
use anyhow::{Context, Result};

use super::super::protocol::PayloadWithChannel;

pub enum Command {
    Request {
        channel_id: u16,
    },
    Release {
        channel_id: u16,
    },
    // From client to proxy, signals that channel is closed (either by client or server)
    ChannelClosed {
        channel_id: u16,
    },
    // From client to proxy, signals that an error occurred on the channel, so it can be closed and cleaned up by proxy
    // Sends the packet that could not be sent, so we can resent it if the error is recoverable (e.g. temporary network issue)
    ChannelError {
        packet: Option<PayloadWithChannel>,
        message: String,
    },
    // Used internally by proxy to signal server close or error, not sent by handler
    ClientClose,
    ClientError {
        message: String,
    },

    // Crypt related
    UpdateSeq(u64, u64),
}

pub struct Handler {
    ctrl_tx: flume::Sender<Command>,
}

impl Handler {
    pub fn new(ctrl_tx: flume::Sender<Command>) -> Self {
        Self { ctrl_tx }
    }

    pub async fn request_channel(&self, channel_id: u16) -> Result<()> {
        self.ctrl_tx
            .send_async(Command::Request { channel_id })
            .await
            .context("Failed to send request channel command")
    }

    pub async fn release_channel(&self, channel_id: u16) -> Result<()> {
        self.ctrl_tx
            .send_async(Command::Release { channel_id })
            .await
            .context("Failed to send release channel command")
    }

    pub async fn channel_closed(&self, channel_id: u16) -> Result<()> {
        self.ctrl_tx
            .send_async(Command::ChannelClosed { channel_id })
            .await
            .context("Failed to send channel closed command")
    }

    pub async fn channel_error(
        &self,
        packet: Option<PayloadWithChannel>,
        message: String,
    ) -> Result<()> {
        self.ctrl_tx
            .send_async(Command::ChannelError { packet, message })
            .await
            .context("Failed to send channel error command")
    }

    pub async fn update_seq(&self, inbound: u64, outbound: u64) -> Result<()> {
        self.ctrl_tx
            .send_async(Command::UpdateSeq(inbound, outbound))
            .await
            .context("Failed to send update seq command")
    }

    pub fn new_command_channel() -> (flume::Sender<Command>, flume::Receiver<Command>) {
        flume::bounded(4)  // No need for more than a few commands buffered, as they are processed sequentially by the handler
    }
}
