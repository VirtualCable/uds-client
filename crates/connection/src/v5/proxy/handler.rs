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

use flume::Sender;

use super::super::protocol::{PayloadReceiver, PayloadWithChannel, PayloadWithChannelSender};

pub struct ServerChannels {
    pub tx: PayloadWithChannelSender,
    pub rx: PayloadReceiver,
}

#[derive(Debug)]
pub enum Command {
    RequestChannel {
        channel_id: u16,
        response: Sender<Result<ServerChannels>>,
    },
    ReleaseChannel {
        channel_id: u16,
    },
    // From client to proxy, signals ordered eof of connection to tunnel
    ConnectionClosed,
    // From client to proxy, signals that an error occurred on the channel, so it can be closed and cleaned up by proxy
    // Sends the packet that could not be sent, so we can resent it if the error is recoverable (e.g. temporary network issue)
    ChannelError {
        packet: Option<PayloadWithChannel>,
        sequence: (u64, u64), // For next crypt recreation
        message: String,
    },
    // Used internally by proxy to signal server close or error, not sent by handler
    ClientClose,
    ClientError {
        message: String,
    },
}

pub struct Handler {
    ctrl_tx: flume::Sender<Command>,
}

impl Handler {
    pub fn new(ctrl_tx: flume::Sender<Command>) -> Self {
        Self { ctrl_tx }
    }

    pub async fn request_channel(&self, channel_id: u16) -> Result<ServerChannels> {
        let (response_tx, response_rx) = flume::bounded(1);
        self.ctrl_tx
            .send_async(Command::RequestChannel {
                channel_id,
                response: response_tx,
            })
            .await
            .context("Failed to send request channel command")?;

        // Wait for the response with timeout
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                Err(anyhow::anyhow!("Timeout waiting for channel response"))
            }
            result = response_rx.recv_async() => {
                result.context("Failed to receive channel response")?
            }
        }
    }

    pub async fn release_channel(&self, channel_id: u16) -> Result<()> {
        self.ctrl_tx
            .send_async(Command::ReleaseChannel { channel_id })
            .await
            .context("Failed to send release channel command")
    }

    pub async fn connection_closed(&self) -> Result<()> {
        self.ctrl_tx
            .send_async(Command::ConnectionClosed)
            .await
            .context("Failed to send connection closed command")
    }

    pub async fn channel_error(
        &self,
        packet: Option<PayloadWithChannel>,
        sequence: (u64, u64),
        message: String,
    ) -> Result<()> {
        self.ctrl_tx
            .send_async(Command::ChannelError {
                packet,
                sequence,
                message,
            })
            .await
            .context("Failed to send channel error command")
    }

    pub fn new_command_channel() -> (flume::Sender<Command>, flume::Receiver<Command>) {
        flume::bounded(4) // No need for more than a few commands buffered, as they are processed sequentially by the handler
    }
}
