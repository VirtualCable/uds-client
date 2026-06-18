// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
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
                // Stop signal
                _ = self.stop.wait_async() => {
                    break;
                }
                // Read from socket
                result = self.reader.read(&mut buffer) => {
                    match result {
                        Ok(0) => {
                            // EOF, stop the server
                            log::debug!("Client closed the connection, stopping tunnel server");
                            self.proxy_ctrl.release_channel(self.channel_id).await?;
                            break;
                        }
                        Ok(n) => {
                            // Send to proxy, if error, no proxy so no notification of release channel
                            // Note: This may trigger stop for stopping the full tunnel processes group
                            if let Err(e) = self.send_data(&PayloadWithChannel::new(self.channel_id, &buffer[..n])).await {
                                 // Try to release channel, but ignore error, as we are already in error state
                                let _ = self.proxy_ctrl.release_channel(self.channel_id).await;
                                log::debug!("Failed to send data to proxy: {:?}", e.to_string());
                                return Err(e);
                            }
                        }
                        Err(_e) => {
                            // May be normal. RDP client (mstsc) may close the connection without notice, so we just log and exit,
                            // no error, as this means the client is not running, so we simply exit
                            // Try to release channel, but ignore error, as we are already in error state
                            let _ = self.proxy_ctrl.release_channel(self.channel_id).await;
                            #[cfg(debug_assertions)]
                            log::debug!("Stopping tunnel server due to local error: {:?}", _e.to_string());

                            break;
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
            }
        }
        log::debug!("Tunnel server stopped");
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
