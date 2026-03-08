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
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use shared::{log, system::trigger::Trigger};

use crypt::tunnel::{Crypt, types::PacketBuffer};

use super::{
    protocol::{Command, PayloadWithChannel, PayloadWithChannelReceiver, PayloadWithChannelSender},
    proxy::{Handler, RecoveryBuffer},
};

pub struct TunnelClientInboundStream<R>
where
    R: AsyncReadExt + Unpin + 'static,
{
    reader: R,
    tx: PayloadWithChannelSender,
    crypt: Crypt,
    stop: Trigger,
}

impl<R> TunnelClientInboundStream<R>
where
    R: AsyncReadExt + Unpin + 'static,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(reader: R, tx: PayloadWithChannelSender, crypt: Crypt, stop: Trigger) -> Self {
        Self {
            reader,
            tx,
            crypt,
            stop,
        }
    }

    async fn run(&mut self) -> Result<()> {
        let mut buffer = PacketBuffer::new();
        loop {
            tokio::select! {
                       _ = self.stop.wait_async() => {
                           // The only exit point that does not notifies
                           break;
                       }
                       // Note: read of crypt_inbound is not cancel safe, but
                       // in case of stop (the only other branch), we don't mind as long as it finishes.
                       packet = self.crypt.read(&self.stop, &mut self.reader, &mut buffer) => {
                           match packet {
                               Ok((decrypted_data, channel)) => {
                                   if decrypted_data.is_empty() && !self.stop.is_triggered() {
                                       log::info!("Tunnel server closed connection");
                                       break;
                                   }

                                   let payload = PayloadWithChannel {
                                       channel_id: channel,
                                       payload: decrypted_data.into(),
                                   };

                                   if self.tx.send_async(payload).await.is_err() {
                                       log::debug!("Proxy channel closed. Exiting inbound");
                                       break;
                                   }
                               }
                               Err(e) => {
                                   log::error!("Inbound crypt read failed: {:?}", e);
                                   break;
                               }
                           }
                   }
            }
        }
        // Stop the other tunnel client side (for try_join! to finish)
        self.stop.trigger();

        Ok(())
    }
}

pub struct TunnelClientOutboundStream<W>
where
    W: AsyncWriteExt + Unpin + 'static,
{
    writer: W,
    rx: PayloadWithChannelReceiver,
    crypt: Crypt,
    stop: Trigger,
}

impl<W> TunnelClientOutboundStream<W>
where
    W: AsyncWriteExt + Unpin + 'static,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new(writer: W, rx: PayloadWithChannelReceiver, crypt: Crypt, stop: Trigger) -> Self {
        Self {
            writer,
            rx,
            crypt,
            stop,
        }
    }

    pub async fn recover_buffer(&mut self, recovery_buffer: &RecoveryBuffer) -> Result<()> {
        log::debug!("Resending unsent packet for session in client outbound stream");
        // Send all unsent packets
        while let Some(unsent_packet) = recovery_buffer.get().take_unsent_packet() {
            // We can block here because we are already in the connection task, and we want to ensure the unsent packet is sent before processing new packets
            // If we fail to send, we will retry on next connection, so it's not critical to send it on this connection
            self.send_data(&unsent_packet).await?;
        }
        Ok(())
    }

    async fn run(&mut self, recovery_buffer: RecoveryBuffer) -> Result<()> {
        self.recover_buffer(&recovery_buffer).await?;
        loop {
            tokio::select! {
                    biased;  // Prefer stop over receiving, and avoid using randomness of select
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
                        // Note that failed packet is raised with the error
                        let data = recovery_buffer.get().push(self.crypt.current_seq() + 1, packet)?;
                        self.send_data(data).await?;

                        if data.channel_id == 0 {
                            #[cfg(debug_assertions)]
                            log::debug!("Received packet for channel 0: {:?}", Command::try_from(data.clone()));
                            // Note: If we receive a close command, stop
                            if let Ok(msg) = Command::try_from(data.clone()) && let Command::Close = msg {
                                log::debug!("Received close command, stopping tunnel client");
                                break;
                            }
                        }
                    }
            }
        }
        // Tunnel should be set so other sides can exit if they are waiting for it
        self.stop.trigger();
        Ok(())
    }

    // so the client can reconnect to server
    async fn send_data(&mut self, data: &PayloadWithChannel) -> Result<()> {
        self.crypt
            .write(&self.stop, &mut self.writer, data.channel_id, &data.payload)
            .await
    }
}

async fn global_to_local_stop(global_stop: Trigger, local_stop: Trigger) -> Result<()> {
    // This is a placeholder for any global to local stop signal handling if needed in the future
    tokio::select! {
        _ = global_stop.wait_async() => {
            local_stop.trigger();
        }
        _ = local_stop.wait_async() => {
            // Local stop triggered, just return
        }
    }
    Ok(())
}

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

    pub async fn run(self, recovery_buffer: RecoveryBuffer) {
        let local_stop = Trigger::new();

        let mut inbound = TunnelClientInboundStream::new(
            self.reader,
            self.tx,
            self.crypt_inbound,
            local_stop.clone(),
        );
        let mut outbound = TunnelClientOutboundStream::new(
            self.writer,
            self.rx,
            self.crypt_outbound,
            local_stop.clone(),
        );

        let err_msg = if let Err(e) = tokio::try_join!(
            inbound.run(),
            outbound.run(recovery_buffer),
            global_to_local_stop(self.stop, local_stop)
        ) {
            log::error!("Tunnel client error: {:?}", e.to_string());
            Some(e.to_string())
        } else {
            None
        };

        if let Err(e) = self
            .proxy_ctrl
            .client_result(
                (inbound.crypt.current_seq(), outbound.crypt.current_seq()),
                err_msg.unwrap_or_default(),
            )
            .await
        {
            log::error!("Failed to send client result: {:?}", e);
        }

        log::info!("Tunnel client stopped");
    }
}

// Tests module
#[cfg(test)]
mod tests;
