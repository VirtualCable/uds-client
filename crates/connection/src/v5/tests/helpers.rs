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

#[cfg(test)]
pub use test_helpers::*;

#[cfg(test)]
mod test_helpers {
    use anyhow::Result;
    use tokio::io::AsyncReadExt;

    use shared::{log, system::trigger::Trigger};

    use crypt::{
        secrets::{CryptoKeys, derive_tunnel_material, get_tunnel_crypts},
        tunnel::types::PacketBuffer,
        types::{SharedSecret, Ticket},
    };

    use super::super::protocol::{
        PayloadWithChannel, PayloadWithChannelReceiver, PayloadWithChannelSender,
        consts::HANDSHAKE_V2_SIGNATURE,
    };

    // Helper to create dummy ticket, ensure always the same
    pub fn dummy_ticket() -> Ticket {
        Ticket::new([b'x'; 48])
    }

    // Helper to create dummy shared secret
    pub fn dummy_shared_secret() -> SharedSecret {
        SharedSecret::new([0u8; 32])
    }

    pub fn dummy_crypt_info() -> CryptoKeys {
        derive_tunnel_material(&dummy_shared_secret(), &dummy_ticket()).unwrap()
    }

    pub struct RemoteServer {
        pub listen_host: String,
        pub listen_port: u16,
        pub stop: Trigger,
        pub rx: PayloadWithChannelReceiver,
        pub tx: PayloadWithChannelSender,
    }

    impl RemoteServer {
        pub fn listen_address(&self) -> String {
            format!("{}:{}", self.listen_host, self.listen_port)
        }
    }

    pub async fn remote_server_dispatcher(
        stop: Trigger,
        mut socket: tokio::net::TcpStream,
        rx: PayloadWithChannelReceiver,
        tx: PayloadWithChannelSender,
    ) -> Result<()> {
        let (mut crypt_output, mut crypt_input) =
            get_tunnel_crypts(&dummy_crypt_info(), (0, 0)).unwrap();

        let ticket = dummy_ticket();
        let mut buf = PacketBuffer::new();
        // Read handshake, but do not check it, just skip for tests
        // Do not check real data received, that has it specific test elsewhere
        {
            let handshake_buf = &mut [0u8; HANDSHAKE_V2_SIGNATURE.len() + 1 + 48]; // Handshake header + cmd + ticket
            socket.read_exact(handshake_buf).await?;
            // Now read encripted ticket again, but do not check it, just skip for tests
            crypt_input.read(&stop, &mut socket, &mut buf).await?;
        }
        crypt_output
            .write(&stop, &mut socket, 0, ticket.as_ref())
            .await?;

        loop {
            tokio::select! {
                _ = stop.wait_async() => {
                    log::debug!("Stop signal received, shutting down remote server dispatcher");
                    return Ok(());
                }
                data = crypt_input.read(&stop, &mut socket, &mut buf) => {
                    log::debug!("Data received: {:?}", data);
                    // Decrypt data
                    let (data, channel_id) = data?;
                    if data.is_empty() {
                        log::info!("Client closed the connection");
                        return Ok(());
                    }
                    // Send data back, same as received and to tx
                    tx.send_async(PayloadWithChannel::new(channel_id, data)).await?;
                }
                channel_data = rx.recv_async() => {
                    log::debug!("Data received from channel: {:?}", channel_data);
                    let data = channel_data?;
                    crypt_output.write(&stop, &mut socket, data.channel_id, &data.payload).await?;
                }
            }
        }
    }

    pub async fn dummy_remote_server() -> RemoteServer {
        let stop = Trigger::new();

        let listener = crate::utils::create_listener(None, false).await.unwrap();
        // Adress without port, to be used in handshake
        let address = listener.local_addr().unwrap().ip().to_string();
        let port = listener.local_addr().unwrap().port();
        let (tx, server_rx) = flume::unbounded();
        let (server_tx, rx) = flume::unbounded();

        tokio::spawn({
            let stop = stop.clone();
            async move {
                loop {
                    tokio::select! {
                        _ = stop.wait_async() => {
                            log::debug!("Stop signal received, shutting down dummy remote server");
                        }
                        accepted = listener.accept() => {
                            log::debug!("**** Incoming connection to dummy remote server");
                            match accepted {
                                Ok((socket, _)) => {
                                    tokio::spawn({
                                        let stop = stop.clone();
                                        let server_rx = server_rx.clone();
                                        let server_tx = server_tx.clone();
                                        async move {
                                            log::debug!("Client connected to dummy remote server");
                                            if let Err(e) = remote_server_dispatcher(stop, socket, server_rx, server_tx).await {
                                                log::error!("Error in remote server dispatcher: {:?}", e);
                                            }
                                            log::debug!("Client disconnected from dummy remote server");
                                        }
                                    });
                                }
                                Err(e) => {
                                    log::error!("Error accepting connection: {:?}", e);
                                    stop.trigger();
                                }
                            }
                        }
                    }
                }
            }
        });

        log::debug!("Dummy remote server listening on {}:{}", address, port);

        RemoteServer {
            listen_host: address,
            listen_port: port,
            stop,
            rx,
            tx,
        }
    }
}
