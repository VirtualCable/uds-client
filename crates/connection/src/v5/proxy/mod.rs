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
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use shared::{log, system::trigger::Trigger};

use crypt::{
    secrets::CryptoKeys, secrets::get_tunnel_crypts, tunnel::types::PacketBuffer, types::Ticket,
};

use super::{
    client::TunnelClient,
    protocol::{
        PayloadWithChannel, PayloadWithChannelReceiver, PayloadWithChannelSender,
        handshake::Handshake, payload_with_channel_pair,
    },
};

mod handler;
mod open_response;
mod servers;

pub use handler::{Command, Handler, ServerChannels};

pub struct Proxy {
    tunnel_server: String, // Host:port of tunnel server to connect to
    ticket: Ticket,
    crypt_info: CryptoKeys,
    stop: Trigger,
    initial_timeout: std::time::Duration,

    // We need to keep track of the seqs for crypt
    // for conneciton recovery
    seqs: (u64, u64),

    // Channels for comms with the client side (the one that will connect to the tunnel server)
    client_tx: PayloadWithChannelSender, // For sending messages to the client side
    client_tx_receiver: PayloadWithChannelReceiver, // Receiver for the client

    client_rx_sender: PayloadWithChannelSender, // Sender for the client
    client_rx: PayloadWithChannelReceiver,      // For receiving messages from the client side

    recover_connection: bool,
    recovery_packet: Option<PayloadWithChannel>,

    servers: servers::ServerChannels,
}

impl Proxy {
    pub fn new(
        tunnel_server: &str,
        ticket: Ticket,
        crypt_info: CryptoKeys,
        initial_timeout: Duration,
        stop: Trigger,
    ) -> Self {
        // Client side channels
        let (tx, tx_receiver) = payload_with_channel_pair();
        let (rx_sender, rx) = payload_with_channel_pair();

        Self {
            tunnel_server: tunnel_server.to_string(),
            ticket,
            crypt_info,
            stop,
            initial_timeout,
            seqs: (0, 0),
            client_tx: tx,
            client_tx_receiver: tx_receiver,
            client_rx: rx,
            client_rx_sender: rx_sender,
            recover_connection: false,
            recovery_packet: None,
            servers: servers::ServerChannels::new(),
        }
    }

    async fn connect(
        &mut self,
        ctrl_tx: &flume::Sender<handler::Command>,
    ) -> Result<TunnelClient<OwnedReadHalf, OwnedWriteHalf>> {
        // Try to connect to tunnel server and authenticate using the ticket and shared secret
        let stream = tokio::time::timeout(
            self.initial_timeout,
            tokio::net::TcpStream::connect(&self.tunnel_server),
        )
        .await?
        .context("Failed to connect to tunnel server")?;

        log::debug!("Connected to tunnel server at {}", self.tunnel_server);

        // Try to disable Nagle's algorithm for better performance in our case
        stream.set_nodelay(true).ok();

        // Create the crypt pair
        let (mut inbound_crypt, mut outbound_crypt) =
            get_tunnel_crypts(&self.crypt_info, self.seqs)?;

        // Send open tunnel command with the ticket and shared secret
        let handshake = if self.recover_connection {
            Handshake::Recover {
                ticket: self.ticket,
            }
        } else {
            self.recover_connection = true; // Next time we will try to recover the connection
            Handshake::Open {
                ticket: self.ticket,
            }
        };
        // Split the stream into reader and writer for easier handling on the next steps
        let (mut reader, mut writer) = stream.into_split();

        log::debug!("Sending handshake to tunnel server");
        handshake
            .write(&mut writer)
            .await
            .context("Failed to send handshake")?;

        log::debug!("Sending handshake ticket to tunnel server");
        // Send the encrypted ticket now to channel 0
        outbound_crypt
            .write(&self.stop, &mut writer, 0, self.ticket.as_ref())
            .await
            .context("Failed to send handshake ticket")?;

        // Read the response, should be the "reconnect" ticket, just in case some connection error
        log::debug!("Waiting for handshake response from tunnel server");
        let mut buffer = PacketBuffer::new();
        let (response, channel_id) = inbound_crypt
            .read(&self.stop, &mut reader, &mut buffer)
            .await
            .context("Failed to read handshake response")?;

        let open_response = open_response::OpenResponse::try_from(response)
            .context("Failed to parse handshake response")?;

        log::debug!(
            "Received handshake response from tunnel server, channel_id: {}, open_response: {:?}",
            channel_id,
            open_response
        );

        // Channel id should be 0 for handshake response, if not, something went wrong
        if channel_id != 0 {
            return Err(anyhow::anyhow!(
                "Expected handshake response on channel 0, got channel {}",
                channel_id
            ));
        }

        log::debug!(
            "Received handshake response from tunnel server, reconnect ticket: {:?}",
            open_response
        );

        // Store reconnect ticket for future use.
        // This is different from original, and different for every conection
        self.ticket = open_response.session_id;

        log::debug!(
            "Received handshake response, reconnect ticket: {:?}",
            self.ticket
        );

        // Create the server and run it in a separate task
        Ok(TunnelClient::new(
            reader,
            writer,
            self.client_rx_sender.clone(),
            self.client_tx_receiver.clone(),
            inbound_crypt,
            outbound_crypt,
            self.stop.clone(),
            handler::Handler::new(ctrl_tx.clone()),
        ))
    }

    // Launchs (or relaunchs) the tunnel server, returns a handler to send commands to the server
    async fn launch_client(&mut self, ctrl_tx: flume::Sender<handler::Command>) -> Result<()> {
        let server = self.connect(&ctrl_tx).await?;
        let recovery_packet = self.recovery_packet.take();
        tokio::spawn(async move {
            if let Err(e) = server.run(recovery_packet).await {
                log::warn!("Tunnel server error: {:?}", e);
                ctrl_tx
                    .send_async(handler::Command::ClientError {
                        message: format!("{:?}", e),
                    })
                    .await
                    .ok();
            } else {
                ctrl_tx.send_async(handler::Command::ClientClose).await.ok();
            }
        });
        Ok(())
    }

    pub async fn run(mut self) -> Result<Handler> {
        let (ctrl_tx, ctrl_rx) = Handler::new_command_channel();

        // Launch server or return an error
        self.launch_client(ctrl_tx.clone()).await?;

        // Launch the main proxy task
        tokio::spawn({
            let ctrl_tx = ctrl_tx.clone();
            async move {
                if let Err(e) = self.run_task(ctrl_tx, ctrl_rx).await {
                    log::error!("Proxy run error: {:?}", e);
                }
            }
        });

        Ok(handler::Handler::new(ctrl_tx))
    }

    pub async fn run_task(
        mut self,
        ctrl_tx: flume::Sender<Command>,
        ctrl_rx: flume::Receiver<Command>,
    ) -> Result<()> {
        // Execute the proxy task
        // Main loop to handle tunnel communication, moves self into the async task
        loop {
            tokio::select! {
                // Check for stop signal
                _ = self.stop.wait_async() => {
                    break;
                }

                // Handle control commands
                cmd = ctrl_rx.recv_async() => {
                    match cmd {
                        Ok(cmd) => {
                            if let Err(e) = self.handle_command(cmd, &ctrl_tx).await {
                                log::error!("Error handling command: {:?}", e);
                                break;
                            }
                        }
                        Err(_) => {
                            // Control channel closed, we should stop
                            break;
                        }
                    }
                }
                msg = self.servers.recv() => {
                    match msg {
                        Ok(msg) => {
                            if let Err(e) = self.client_tx.send_async(msg).await {
                                log::error!("Error sending message to channel: {:?}", e);
                            }
                        }
                        Err(_) => {
                            // Server channel closed, we should stop
                            break;
                        }
                    }
                }
                msg = self.client_rx.recv_async() => {
                    let msg = msg.context("Failed to receive message from channel")?;
                    if let Err(e) = self.servers.send_to_channel(msg).await {
                        log::error!("Error sending message to server: {:?}", e);
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_command(
        &mut self,
        cmd: handler::Command,
        ctrl_tx: &flume::Sender<handler::Command>,
    ) -> Result<()> {
        match cmd {
            handler::Command::RequestChannel {
                channel_id,
                response,
            } => {
                // Register a new server, and return the comms channel for it
                self.client_tx
                    .send_async(super::protocol::Command::OpenChannel { channel_id }.to_message())
                    .await
                    .context("Failed to send open channel command to client")?;
                let (tx, rx) = self.servers.register_server(channel_id).await?;
                response
                    .send_async(Ok(handler::ServerChannels { tx, rx }))
                    .await?;
            }
            handler::Command::ReleaseChannel { channel_id } => {
                self.servers.close_server(channel_id);
            }
            handler::Command::ConnectionClosed => {
                // Stop all servers
                self.servers.stop_all_servers();
                log::info!(
                    "Received connection closed command from client, will attempt to reconnect"
                );
                self.stop.trigger(); // Stop current server, which will in turn exit run loop
            }
            handler::Command::ChannelError {
                packet,
                message,
                sequence,
            } => {
                self.recovery_packet = packet;
                self.seqs = sequence;
                log::error!(
                    "Channel error: {}, packet for recovery: {:?}",
                    message,
                    self.recovery_packet
                );
                // TODO: Maybe a wait before relaunching?
                self.launch_client(ctrl_tx.clone()).await?;
            }
            handler::Command::PacketError => {
                log::error!("Packet error, will attempt to reconnect")
                // Close server and relaunch a new one
            }
            handler::Command::ClientClose => {
                self.stop.trigger();
            }
            handler::Command::ClientError { message } => {
                log::error!("Client error: {}", message);
                self.stop.trigger();
            }
        }
        Ok(())
    }
}

// Tests module
#[cfg(test)]
mod tests;
