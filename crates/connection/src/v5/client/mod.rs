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
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use shared::{log, system::trigger::Trigger};

use crypt::tunnel::{Crypt, consts::CRYPT_PACKET_SIZE, types::PacketBuffer};

use super::{
    protocol::{PayloadWithChannel, PayloadWithChannelReceiver, PayloadWithChannelSender},
    proxy::Handler,
};

pub struct TunnelClient<R, W>
where
    R: AsyncReadExt + Unpin + 'static,
    W: AsyncWriteExt + Unpin + 'static,
{
    reader: R,
    writer: W,

    tx: PayloadWithChannelSender,
    rx: PayloadWithChannelReceiver,

    crypt_inbound: Crypt,
    crypt_outbound: Crypt,

    stop: Trigger,
    proxy_ctrl: Handler,
}

impl<R, W> TunnelClient<R, W>
where
    R: AsyncReadExt + Unpin + 'static,
    W: AsyncWriteExt + Unpin + 'static,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        reader: R,
        writer: W,
        tx: PayloadWithChannelSender,
        rx: PayloadWithChannelReceiver,
        crypt_inbound: Crypt,
        crypt_outbound: Crypt,
        stop: Trigger,
        proxy_ctrl: Handler,
    ) -> Self {
        Self {
            reader,
            writer,
            tx,
            rx,
            crypt_inbound,
            crypt_outbound,
            stop,
            proxy_ctrl,
        }
    }

    pub async fn run(mut self, initial_packet: Option<PayloadWithChannel>) -> Result<()> {
        let mut buffer = PacketBuffer::new();
        if let Some(packet) = initial_packet {
            // We have an initial packet, process it first
            log::debug!("Processing initial recovery packet before entering main loop");
            self.send_data(&packet).await?;
        }
        loop {
            tokio::select! {
                    _ = self.stop.wait_async() => {
                        // The only exit point that does not notifies
                        break;
                    }
                    packet = self.rx.recv_async() => {
                        let packet = match packet {
                            Ok(p) => p,
                            Err(e) => {
                                // This means the proxy is not running, so we simply exit
                                log::debug!("Proxy stopped. Exiting tunnel client.: {:?}", e.to_string());
                                break;
                            }
                        };
                        self.send_data(&packet).await?;
                    }
                    packet = self.crypt_inbound.read(&self.stop, &mut self.reader, &mut buffer) => {
                        let (decrypted_data, channel) = match packet {
                            Ok((data, channel)) => (data, channel),
                            Err(e) => {
                                // This can be an "internal" error (like decryption failure) or an "external" one (like connection closed). In both cases we log it and stop the client, but only in the first case we notify the proxy with a ChannelError command, as in the second case the connection is already closed and the proxy will be notified by the connection closure.
                                self.proxy_ctrl
                                    .packet_error()
                                    .await
                                    .ok();
                                log::error!("Failed to read packet from tunnel server: {:?}", e);
                                break;
                            }
                        };
                        log::debug!("Received packet from tunnel server: channel_id={}, payload_size={}", channel, decrypted_data.len());
                        // if decrypted_data is empty, it means the connection was closed
                        if decrypted_data.is_empty() && !self.stop.is_triggered() {
                            log::info!("Tunnel server closed the connection");
                            self.proxy_ctrl
                                .connection_closed()
                                .await
                                .ok(); // Notify proxy of connection closure correctly
                            break;
                        }
                        // Send to proxy
                        if self.tx.send_async(super::protocol::PayloadWithChannel {
                            channel_id: channel,
                            payload: decrypted_data.into(),
                        }).await.is_err() {
                            // This means the proxy is not running, so we simply exit
                            log::debug!("Proxy stopped. Exiting tunnel client.: {:?}", decrypted_data);
                            break;
                        }
                    }
            }
        }

        Ok(())
    }

    async fn send_data(&mut self, data: &PayloadWithChannel) -> Result<()> {
        let mut offset = 0;

        let payload = data.payload.as_ref();
        // Divide data into CRYPT_PACKET_SIZE chunks and send them
        while offset < payload.len() {
            log::debug!(
                "Sending packet chunk to tunnel server: channel_id={}, offset={}, chunk_size={}",
                data.channel_id,
                offset,
                (payload.len() - offset).min(CRYPT_PACKET_SIZE),
            );
            let end = (offset + CRYPT_PACKET_SIZE).min(payload.len());
            let chunk = &payload[offset..end];
            if let Err(e) = self
                .crypt_outbound
                .write(&self.stop, &mut self.writer, data.channel_id, chunk)
                .await
            {
                self.proxy_ctrl
                    .channel_error(
                        Some(data.clone()),
                        (
                            self.crypt_inbound.current_seq(),
                            self.crypt_outbound.current_seq(),
                        ),
                        format!("Failed to write packet chunk to tunnel server: {:?}", e),
                    )
                    .await
                    .ok();
                log::error!("Failed to write packet chunk to tunnel server: {:?}", e);
                return Err(e).context("Failed to write packet chunk to tunnel server");
            }
            offset = end;
        }

        Ok(())
    }
}

// Tests module
#[cfg(test)]
mod tests;
