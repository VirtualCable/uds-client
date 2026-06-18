// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::{Context, Result};

use flume::Sender;

use super::super::{
    log,
    protocol::{PayloadReceiver, PayloadWithChannelSender},
};

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
    // From client to proxy, signals that an error occurred on the channel, so it can be closed and cleaned up by proxy
    // Sends the packet that could not be sent, so we can resent it if the error is recoverable (e.g. temporary network issue)
    ClientResult {
        sequence: (u64, u64), // For next crypt recreation
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct Handler {
    ctrl_tx: flume::Sender<Command>,
}

impl Handler {
    pub fn new(ctrl_tx: flume::Sender<Command>) -> Self {
        Self { ctrl_tx }
    }

    pub async fn request_channel(&self, channel_id: u16) -> Result<ServerChannels> {
        log::debug!("Requesting channel {}", channel_id);
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
        log::debug!("Releasing channel {}", channel_id);
        self.ctrl_tx
            .send_async(Command::ReleaseChannel { channel_id })
            .await
            .context("Failed to send release channel command")
    }

    pub async fn client_result(&self, sequence: (u64, u64), message: String) -> Result<()> {
        self.ctrl_tx
            .send_async(Command::ClientResult { sequence, message })
            .await
            .context("Failed to send client result command")
    }

    pub fn new_command_channel() -> (flume::Sender<Command>, flume::Receiver<Command>) {
        flume::bounded(4) // No need for more than a few commands buffered, as they are processed sequentially by the handler
    }
}
