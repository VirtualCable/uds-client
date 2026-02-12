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

use crate::protocol::PayloadWithChannel;

use super::proxy::Handler;

use super::{
    crypt::{Crypt, types::PacketBuffer},
    protocol::{PayloadWithChannelReceiver, PayloadWithChannelSender},
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
    proxy: Handler,
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
        proxy: Handler,
    ) -> Self {
        Self {
            reader,
            writer,
            tx,
            rx,
            crypt_inbound,
            crypt_outbound,
            stop,
            proxy,
        }
    }

    pub async fn write_packet(&mut self, packet: &PayloadWithChannel) -> Result<()> {
        match self
            .crypt_outbound
            .write(
                &self.stop,
                &mut self.writer,
                packet.channel_id,
                packet.payload.as_ref(),
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                self.proxy
                    .channel_error(
                        Some(packet.clone()),
                        format!("Failed to write initial packet to tunnel server: {:?}", e),
                    )
                    .await
                    .ok();
                log::error!("Failed to write initial packet to tunnel server: {:?}", e);
                Err(e).context("Failed to write initial packet to tunnel server")
            }
        }
    }

    pub async fn run(mut self, initial_packet: Option<PayloadWithChannel>) -> Result<()> {
        let mut buffer = PacketBuffer::new();
        if let Some(packet) = initial_packet {
            // We have an initial packet, process it first
            self.write_packet(&packet).await?;
        }
        loop {
            tokio::select! {
                    _ = self.stop.wait_async() => {
                        // The only exit point that does not notifies
                        break;
                    }
                    packet = self.rx.recv_async() => {
                        self.write_packet(&packet.context("Failed to receive packet from proxy")?).await?;
                    }
                    packet = self.crypt_inbound.read(&self.stop, &mut self.reader, &mut buffer) => {
                        let (decrypted_data, channel) = packet.context("Failed to read packet from tunnel server")?;
                        // if decrypted_data is empty, it means the connection was closed
                        if decrypted_data.is_empty() && !self.stop.is_triggered() {
                            log::info!("Tunnel server closed the connection");
                            self.proxy
                                .connection_closed()
                                .await
                                .ok(); // Notify proxy of connection closure correctly
                            break;
                        }
                        // Send to proxy
                        if let Err(e) = self.tx.send_async(super::protocol::PayloadWithChannel {
                            channel_id: channel,
                            payload: decrypted_data.into(),
                        }).await {
                            // This is an "internal" error, as it means the proxy is not processing commands, so we just log it and stop the client
                            log::error!("Failed to send packet to proxy: {:?}", e);
                            break;
                        }
                    }
            }
        }

        Ok(())
    }
}

// Tests module
#[cfg(test)]
mod tests;
