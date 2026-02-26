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
use crypt::tunnel::consts::CRYPT_PACKET_SIZE;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use shared::{log, system::trigger::Trigger};

use super::{
    protocol::{PayloadReceiver, PayloadWithChannel, PayloadWithChannelSender},
    proxy::Handler,
};

// Tunnel server side implementation
// This receives the data from the socket and forwards it to the proxy.
// Also receives data from the proxy and sends it to the socket
pub struct TunnelServer<R, W>
where
    R: AsyncReadExt + Unpin + Send + 'static,
    W: AsyncWriteExt + Unpin + Send + 'static,
{
    reader: R,
    writer: W,

    channel_id: u16,

    tx: PayloadWithChannelSender,
    rx: PayloadReceiver,

    stop: Trigger,
    proxy_ctrl: Handler,
}

impl<R, W> TunnelServer<R, W>
where
    R: AsyncReadExt + Unpin + Send + 'static,
    W: AsyncWriteExt + Unpin + Send + 'static,
{
    pub fn new(
        reader: R,
        writer: W,
        channel_id: u16,
        tx: PayloadWithChannelSender,
        rx: PayloadReceiver,
        stop: Trigger,
        proxy_ctrl: Handler,
    ) -> Self {
        Self {
            reader,
            writer,
            channel_id,
            tx,
            rx,
            stop,
            proxy_ctrl,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        // We can use a bigger buffer, because client will split data into CRYPT_PACKET_SIZE chunks
        let mut buffer = [0u8; 16384];
        loop {
            tokio::select! {
                // Read from socket
                result = self.reader.read(&mut buffer) => {
                    match result {
                        Ok(0) => {
                            // EOF, stop the server
                            self.proxy_ctrl.release_channel(self.channel_id).await?;
                            break;
                        }
                        Ok(n) => {
                            // Send to proxy, if error, no proxy so no notification of release channel
                            // Note: This may trigger stop for stopping the full tunnel processes group
                            if let Err(e) = self.send_data(&PayloadWithChannel::new(self.channel_id, &buffer[..n])).await {
                                log::error!("Failed to send data to proxy: {:?}", e);
                                return Err(e);
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to read from socket: {:?}", e);
                            self.proxy_ctrl.release_channel(self.channel_id).await?;
                            return Err(e.into());
                        }
                    }
                }

                // Read from proxy
                result = self.rx.recv_async() => {
                    // Error receiving from proxy, stop the server as the proxy connection is losy without notification
                    // Note: This may trigger stop for stopping the full tunnel processes group
                    let payload = match result {
                        Ok(payload) => payload,
                        Err(_) => {
                            log::debug!("Proxy stopped. Exiting tunnel server.");
                            return Ok(());  // Just exit, no error, as this means the proxy is not running, so we simply exit
                        }
                    };
                    // Write to socket
                    self.writer.write_all(&payload).await?;
                }

                // Stop signal
                _ = self.stop.wait_async() => {
                    break;
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
            let end = (offset + CRYPT_PACKET_SIZE).min(payload.len());
            let chunk = &payload[offset..end];
            self.tx
                .send_async(PayloadWithChannel::new(data.channel_id, chunk))
                .await?;
            offset = end;
        }

        Ok(())
    }
}

// Tests module
#[cfg(test)]
mod tests;
