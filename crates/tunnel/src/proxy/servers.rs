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
use anyhow::Result;

use shared::{log, system::trigger::Trigger};

use super::super::protocol;

#[derive(Debug, Clone)]
struct ServerInfo {
    sender: protocol::PayloadSender,
    stop: Trigger,
}

pub(super) struct ServerChannels {
    server_senders: Vec<Option<ServerInfo>>,
    sender: protocol::PayloadWithChannelSender,
    receiver: protocol::PayloadWithChannelReceiver,
}

impl ServerChannels {
    pub fn new() -> Self {
        let (sender, receiver) = protocol::payload_with_channel_pair();
        Self {
            server_senders: Vec::new(),
            sender,
            receiver,
        }
    }

    pub async fn register_server(
        &mut self,
        stream_channel_id: u16,
    ) -> Result<flume::Receiver<protocol::Payload>> {
        log::debug!("Creating server for stream_channel_id: {}", stream_channel_id);
        // Ensure vector is large enough
        if self.server_senders.len() < stream_channel_id as usize {
            self.server_senders.resize(stream_channel_id as usize, None);
        }

        // If current server is Some, we are replacing it, so ensure old one receives the stop signal
        if let Some(old_server) = &self.server_senders[(stream_channel_id - 1) as usize] {
            // Ensure notify old client to stop before replacing
            old_server.stop.trigger();
        }

        let (sender, receiver) = protocol::payload_pair();
        // (self.sender.clone(), receiver)

        let stop = Trigger::new();

        self.server_senders[(stream_channel_id - 1) as usize] = Some(ServerInfo { sender, stop });
        Ok(receiver)
    }

    pub async fn send_to_channel(&self, msg: protocol::PayloadWithChannel) -> Result<()> {
        if msg.channel_id == 0 || msg.channel_id as usize > self.server_senders.len() {
            return Err(anyhow::anyhow!(
                "Invalid stream_channel_id: {}",
                msg.channel_id
            ));
        }
        if let Some(client) = &self.server_senders[(msg.channel_id - 1) as usize] {
            client.sender.send_async(msg.payload).await?;
        }
        // If no client, just drop the message
        Ok(())
    }

    pub async fn stop_server(&self, stream_channel_id: u16) {
        if stream_channel_id == 0 || stream_channel_id as usize > self.server_senders.len() {
            return;
        }
        if let Some(server) = &self.server_senders[(stream_channel_id - 1) as usize] {
            server.stop.trigger();
        }
    }

    pub fn stop_all_servers(&self) {
        for server in self.server_senders.iter().flatten() {
            server.stop.trigger();
        }
    }

    pub async fn recv(&self) -> Result<protocol::PayloadWithChannel> {
        let msg = self.receiver.recv_async().await?;
        Ok(msg)
    }

    /// Closes the server for the given stream_channel_id
    pub fn close_server(&mut self, stream_channel_id: u16) {
        if self.server_senders.len() >= stream_channel_id as usize {
            self.server_senders[(stream_channel_id - 1) as usize] = None;
        }
    }
}
