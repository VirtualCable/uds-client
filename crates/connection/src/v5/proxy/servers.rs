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
    ) -> Result<(
        protocol::PayloadWithChannelSender,
        protocol::PayloadReceiver,
    )> {
        log::debug!(
            "Creating server for stream_channel_id: {}",
            stream_channel_id
        );
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
        Ok((self.sender.clone(), receiver))
    }

    /// Closes the server for the given stream_channel_id
    pub fn close_server(&mut self, stream_channel_id: u16) {
        if self.server_senders.len() >= stream_channel_id as usize {
            self.server_senders[(stream_channel_id - 1) as usize] = None;
        }
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

    pub fn stop_all_servers(&self) {
        for server in self.server_senders.iter().flatten() {
            server.stop.trigger();
        }
    }

    pub async fn recv(&self) -> Result<protocol::PayloadWithChannel> {
        let msg = self.receiver.recv_async().await?;
        Ok(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::super::protocol::PayloadWithChannel;

    #[tokio::test]
    async fn test_register_and_communication() {
        let mut channels = ServerChannels::new();
        let channel_id = 1;

        // Register server
        let (tx, rx) = channels.register_server(channel_id).await.unwrap();

        // 1. Test Server -> Proxy (recv)
        let payload = vec![1, 2, 3, 4];
        let msg = PayloadWithChannel::new(channel_id, &payload);

        // Send using the sender returned by register_server
        tx.send_async(msg).await.unwrap();

        // Receive using channels.recv()
        let received = channels.recv().await.unwrap();
        assert_eq!(received.channel_id, channel_id);
        assert_eq!(received.payload.as_ref(), payload.as_slice());

        // 2. Test Proxy -> Server (send_to_channel)
        let response_payload = vec![5, 6, 7, 8];
        let response_msg = PayloadWithChannel::new(channel_id, &response_payload);

        channels.send_to_channel(response_msg).await.unwrap();

        // Receive using the receiver returned by register_server
        let received_response = rx.recv_async().await.unwrap();
        assert_eq!(received_response.as_ref(), response_payload.as_slice());
    }

    #[tokio::test]
    async fn test_multiple_channels() {
        let mut channels = ServerChannels::new();

        let (tx1, rx1) = channels.register_server(1).await.unwrap();
        let (tx2, rx2) = channels.register_server(2).await.unwrap();

        // Send to channel 1
        channels
            .send_to_channel(PayloadWithChannel::new(1, &[10]))
            .await
            .unwrap();
        assert_eq!(rx1.recv_async().await.unwrap().as_ref(), &[10]);

        // Send to channel 2
        channels
            .send_to_channel(PayloadWithChannel::new(2, &[20]))
            .await
            .unwrap();
        assert_eq!(rx2.recv_async().await.unwrap().as_ref(), &[20]);

        // Server 1 sends to proxy
        tx1.send_async(PayloadWithChannel::new(1, &[11]))
            .await
            .unwrap();
        let msg = channels.recv().await.unwrap();
        assert_eq!(msg.channel_id, 1);
        assert_eq!(msg.payload.as_ref(), &[11]);

        // Server 2 sends to proxy
        tx2.send_async(PayloadWithChannel::new(2, &[22]))
            .await
            .unwrap();
        let msg = channels.recv().await.unwrap();
        assert_eq!(msg.channel_id, 2);
        assert_eq!(msg.payload.as_ref(), &[22]);
    }

    #[tokio::test]
    async fn test_invalid_channel() {
        let channels = ServerChannels::new();
        // Channel 0 is invalid
        let err = channels
            .send_to_channel(PayloadWithChannel::new(0, &[]))
            .await;
        assert!(err.is_err());

        // Channel 1 not registered (but vector might be empty, so out of bounds)
        let err = channels
            .send_to_channel(PayloadWithChannel::new(1, &[]))
            .await;
        // If len is 0, 1 > 0, so it returns error.
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_channel_replacement() {
        let mut channels = ServerChannels::new();

        let (_tx1, rx1) = channels.register_server(1).await.unwrap();
        let (_tx2, rx2) = channels.register_server(1).await.unwrap();

        // Send to channel 1
        channels
            .send_to_channel(PayloadWithChannel::new(1, &[99]))
            .await
            .unwrap();

        // rx2 should receive it
        assert_eq!(rx2.recv_async().await.unwrap().as_ref(), &[99]);

        // rx1 should be disconnected because the sender was dropped
        assert!(rx1.recv_async().await.is_err());
    }

    #[tokio::test]
    async fn test_close_server() {
        let mut channels = ServerChannels::new();
        let (_tx, rx) = channels.register_server(1).await.unwrap();

        channels.close_server(1);

        // Sender should be dropped
        assert!(rx.recv_async().await.is_err());

        // Sending to channel 1 should now be ignored (Ok but no action)
        let res = channels
            .send_to_channel(PayloadWithChannel::new(1, &[1]))
            .await;
        assert!(res.is_ok());
    }
}
