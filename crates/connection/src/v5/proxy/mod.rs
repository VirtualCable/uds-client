// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::{cell::UnsafeCell, rc::Rc, sync::atomic::AtomicUsize, time::Duration};

use anyhow::{Context, Result};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

use shared::{log, system::trigger::Trigger};

use crypt::{
    datagram::UdpToken,
    rekey::SessionPrk,
    secrets::{CryptoKeys, TunnelRekeys, get_tunnel_crypts},
    tunnel::types::PacketBuffer,
    types::{SharedSecret, Ticket},
};

use super::{
    client::TunnelClient,
    protocol::{
        Command as ProtoCommand, PayloadWithChannelReceiver, PayloadWithChannelSender,
        handshake::Handshake, payload_with_channel_pair,
    },
};

mod buffer;
mod handler;
pub mod open_response;
mod servers;

pub use {
    buffer::{RecoveryError, RecoverySendBuffer},
    handler::{Command, Handler, ServerChannels},
};

pub static RECOVERY_BUFFER_SIZE: AtomicUsize = AtomicUsize::new(64 * 1024); // Default to 64 KB, can be configured at runtime

#[derive(Debug, Clone)]
pub struct RecoveryBuffer(Rc<UnsafeCell<RecoverySendBuffer>>);

unsafe impl Send for RecoveryBuffer {}
unsafe impl Sync for RecoveryBuffer {}

impl RecoveryBuffer {
    pub fn new(max_bytes: usize) -> Self {
        Self(Rc::new(UnsafeCell::new(RecoverySendBuffer::new(max_bytes))))
    }

    #[allow(clippy::mut_from_ref)]
    pub fn get(&self) -> &mut RecoverySendBuffer {
        unsafe { &mut *self.0.get() }
    }
}

pub struct Proxy {
    tunnel_server: String, // Host:port of tunnel server to connect to
    ticket: Ticket,
    crypt_info: CryptoKeys,
    stop: Trigger,
    initial_timeout: std::time::Duration,

    // We need to keep track of the seqs for crypt
    // for connection recovery
    seqs: (u64, u64),

    // Channels for comms with the client side (the one that will connect to the tunnel server)
    client_tx: PayloadWithChannelSender, // For sending messages to the client side
    client_tx_receiver: PayloadWithChannelReceiver, // Receiver for the client

    client_rx_sender: PayloadWithChannelSender, // Sender for the client
    client_rx: PayloadWithChannelReceiver,      // For receiving messages from the client side

    recover_connection: bool,
    recovery_buffer: RecoveryBuffer,

    client_correctly_closed: bool,

    servers: servers::ServerChannels,

    // Session's rekeying material, adopted from the server's
    // `OpenResponse.rekey_log2`. `k` is `None` until the first successful
    // Open handshake (the handshake frames themselves run in epoch 0 on the
    // legacy keys); once adopted it is pinned for the whole session and
    // re-used on every `Recover` — the server never re-negotiates it, so we
    // must not either. The PRK is derived once from the session's original
    // shared secret and ticket (exactly like the server's `Session`), so
    // epoch >= 1 keys match the far side byte-for-byte.
    rekey_log2: std::sync::Arc<std::sync::OnceLock<u8>>,
    rekey_prk: std::sync::Arc<SessionPrk>,

    // UDP token, relay port and adopted rekey threshold of the current
    // session, extracted from the open response (None until the first
    // successful connect; zero token = UDP disabled, zero port = same as the
    // TCP tunnel port). The threshold rides along so the UDP leg can build
    // its crypts with the very same `k` as the TCP leg.
    udp_token: std::sync::Arc<std::sync::Mutex<Option<(UdpToken, u16, u8)>>>,
}

impl Proxy {
    pub fn new(
        tunnel_server: &str,
        ticket: Ticket,
        crypt_info: CryptoKeys,
        shared_secret: &SharedSecret,
        initial_timeout: Duration,
        stop: Trigger,
    ) -> Self {
        // Client side channels
        let (tx, tx_receiver) = payload_with_channel_pair();
        let (rx_sender, rx) = payload_with_channel_pair();

        // The session PRK is derived from the *original* ticket (the one the
        // broker issued and the launcher presents in the Open handshake),
        // exactly like the server's `Session::with_rekey_log2`. The ticket is
        // replaced by the equivalent session id after the first response, but
        // the PRK never is: epoch >= 1 keys must match the far side.
        // Derived eagerly and cheaply (one HKDF extract); a session that
        // negotiates k = 0 simply never expands it.
        let rekey_prk = std::sync::Arc::new(SessionPrk::derive(shared_secret, &ticket));

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
            recovery_buffer: RecoveryBuffer::new(
                RECOVERY_BUFFER_SIZE.load(std::sync::atomic::Ordering::Relaxed),
            ),
            client_correctly_closed: false,
            servers: servers::ServerChannels::new(),
            rekey_log2: std::sync::Arc::new(std::sync::OnceLock::new()),
            rekey_prk,
            udp_token: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Shared slot with the UDP token, relay port and rekey threshold of the
    /// current session, filled after each successful (re)connect. `None` until
    /// the first connect. The threshold rides along so the UDP leg builds its
    /// crypts with the very same `k` as the TCP leg.
    pub fn udp_token_handle(
        &self,
    ) -> std::sync::Arc<std::sync::Mutex<Option<(UdpToken, u16, u8)>>> {
        self.udp_token.clone()
    }

    /// Builds the session's TCP rekey states for threshold `k`. `k = 0`
    /// collapses both states to a single legacy epoch, so installing it is a
    /// no-op on the wire; the launcher still installs it for symmetry (and to
    /// pin the adopted value). The PRK was derived once in `new`, so the
    /// epoch >= 1 keys match the server byte-for-byte.
    fn tunnel_rekeys(&self, k: u8) -> TunnelRekeys {
        TunnelRekeys::from_parts(&self.crypt_info, self.rekey_prk.clone(), k)
    }

    /// Adopts the server-negotiated threshold into the `OnceLock` (first
    /// writer wins; later recovers keep the pinned session value, exactly as
    /// the server pins it at session creation). Returns the value now in
    /// force.
    fn pin_rekey_log2(&self, k: u8) -> u8 {
        *self.rekey_log2.get_or_init(|| k)
    }

    /// Threshold currently pinned for this session (before the first Open
    /// handshake resolves it, `None` means "not yet negotiated" and the crypts
    /// run on the legacy epoch-0 keys).
    fn adopted_rekey_log2(&self) -> Option<u8> {
        self.rekey_log2.get().copied()
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
        // OS-level liveness backup for the KA frames: see `utils::set_keepalive`.
        crate::utils::set_keepalive(&stream);

        // Create the crypt pair
        let (mut inbound_crypt, mut outbound_crypt) =
            get_tunnel_crypts(&self.crypt_info, self.seqs)?;

        // On a `Recover` the session counters are already past zero, so the
        // handshake ticket frame itself may belong to an epoch > 0. The server
        // always decrypts it with the session's pinned `k` (it never
        // re-negotiates), so we must install the adopted rekey state on the
        // crypts BEFORE writing the ticket. On the very first `Open` there is
        // no pinned value yet: the ticket and the `OpenResponse` both run in
        // epoch 0 on the legacy keys (the server's counters are still at 0),
        // so we adopt `k` only after parsing the response.
        if self.recover_connection
            && let Some(k) = self.adopted_rekey_log2()
        {
            let rekeys = self.tunnel_rekeys(k);
            inbound_crypt.set_rekey(rekeys.inbound);
            outbound_crypt.set_rekey(rekeys.outbound);
        }

        // Send open tunnel command with the ticket and shared secret
        let handshake = if self.recover_connection {
            Handshake::Recover {
                ticket: self.ticket.clone(),
                seqs: self.seqs,
            }
        } else {
            Handshake::Open {
                ticket: self.ticket.clone(),
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
            open_response.session_id
        );

        // Rekeying threshold: the server pins `k` at session creation and
        // must repeat it verbatim on every `Recover`. A mismatch means the
        // two sides would derive different epoch keys while still holding
        // the AEAD-ticket confirm, so both legs are about to walk on
        // incompatible keystreams — the far side is buggy or hostile; fail
        // hard rather than silently corrupt the tunnel. On the first `Open`
        // there is no pinned value yet, so we adopt whatever the server
        // announced.
        let k = match self.rekey_log2.get() {
            Some(&pinned) if pinned != open_response.rekey_log2 => {
                return Err(anyhow::anyhow!(
                    "Server rekey threshold changed mid-session (pinned {}, got {}) — aborting",
                    pinned,
                    open_response.rekey_log2
                ));
            }
            Some(&pinned) => pinned,
            None => self.pin_rekey_log2(open_response.rekey_log2),
        };
        // Adopt the threshold into the crypts before any data frame flows.
        // On the first `Open` the handshake frames just exchanged were epoch
        // 0 on the legacy keys (the server's counters are at their initial
        // value), so installing at this exact seam is invisible on the wire.
        // On a `Recover` the states were already installed before the ticket
        // frame (which itself may sit in an epoch > 0).
        if !self.recover_connection {
            let rekeys = self.tunnel_rekeys(k);
            inbound_crypt.set_rekey(rekeys.inbound);
            outbound_crypt.set_rekey(rekeys.outbound);
        }

        // Store reconnect ticket for future use.
        // This is different from original, and different for every conection
        self.ticket = open_response.session_id;
        // Store the UDP token, relay port and threshold of the session
        // (unchanged across recovers)
        *self.udp_token.lock().unwrap() =
            Some((open_response.udp_token, open_response.udp_port, k));
        // Skip, if recovery, the the already processed packets (note that pre increment we must stop on PREV SEQ)
        // inbound = other side inbound, not our
        if self.recover_connection {
            let recovery_buffer = self.recovery_buffer.get();
            log::debug!(
                "Attempting to recover connection, skipping packets until seq {:?} from {:?}",
                open_response.inbound_seq,
                recovery_buffer,
            );
            recovery_buffer
                .skip(open_response.inbound_seq - 1)
                .context("Failed to skip packets in recovery buffer")?;
            log::debug!(
                "Finished skipping packets for recovery, remaining buffer: {:?}",
                recovery_buffer,
            );
        } else {
            // Next one will be a recovery connection
            self.recover_connection = true; // Next time we will try to recover the connection
        }

        log::debug!(
            "Received handshake response, reconnect ticket: {:?}",
            self.ticket
        );

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

    // Launches (or relaunches) the tunnel client, returns a handler to send commands to the client
    async fn launch_client(&mut self, ctrl_tx: flume::Sender<handler::Command>) -> Result<()> {
        let client = self.connect(&ctrl_tx).await?;
        tokio::spawn(client.run(self.recovery_buffer.clone()));
        Ok(())
    }

    pub async fn run(mut self) -> Result<Handler> {
        let (ctrl_tx, ctrl_rx) = Handler::new_command_channel();

        // Launch client or return an error
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
                biased;
                // Check for stop signal
                _ = self.stop.wait_async() => {
                    break;
                }

                // Handle control commands
                cmd = ctrl_rx.recv_async() => {
                    match cmd {
                        Ok(cmd) => {
                            if let Err(e) = self.handle_ctrl_command(cmd, &ctrl_tx).await {
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
                    // Channel 0 messages are command messages, process them
                    if msg.channel_id == 0 {
                        match ProtoCommand::try_from(msg) {
                            Ok(cmd) => {
                                if let Err(e) = self.handle_proto_command(cmd).await {
                                    log::error!("Error handling command: {:?}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to parse command from channel 0: {:?}", e);
                                break;
                            }
                        }
                        continue;
                    }
                    if let Err(e) = self.servers.send_to_channel(msg).await {
                        log::error!("Error sending message to server: {:?}", e);
                    }
                }
            }
        }
        Ok(())
    }

    async fn handle_proto_command(&mut self, cmd: ProtoCommand) -> Result<()> {
        match cmd {
            ProtoCommand::Close => {
                log::debug!("Received close command from channel 0, will attempt to reconnect");
                // Try also to send back the close command
                let _ = self.client_tx.send_async(ProtoCommand::Close.into()).await;

                // Stop all servers
                self.servers.stop_all_servers();

                // Flag we have been correctly closed by client, so we don't try to reconnect, just stop the proxy
                self.client_correctly_closed = true;
            }
            _ => {
                log::debug!("Received command from channel 0: {:?}", cmd);
                // For now, we just log the command, but we could handle some more commands here if needed
            }
        }
        Ok(())
    }

    async fn handle_ctrl_command(
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
                log::debug!("Processing command release channel {}", channel_id);
                self.servers.close_server(channel_id);
                self.client_tx
                    .send_async(super::protocol::Command::CloseChannel { channel_id }.to_message())
                    .await
                    .context("Failed to send close channel command to client")?;
                // If no server remains (all are closed), send also the Close command to client, so it can cleanup and close all
                if self.servers.is_empty() {
                    log::debug!("No more active channels, sending close command to client");
                    self.client_tx
                        .send_async(super::protocol::Command::Close.into())
                        .await
                        .context("Failed to send close command to client")?;
                    // Flag we have been correctly closed by client, so we don't try to reconnect, just stop the proxy
                    self.client_correctly_closed = true;
                }
            }
            handler::Command::ClientResult { message, sequence } => {
                // If we received the close command from remote, we should not try to reconnect, just stop the proxy
                // If we stopped the server, stopped also will be set, do not try to reconnect in that case either
                if self.stop.is_triggered() || !self.client_correctly_closed {
                    self.seqs = sequence;
                    log::debug!(
                        "Client Result: {}, packet for recovery: {:?}, seqs: {:?}",
                        message,
                        self.recovery_buffer,
                        self.seqs,
                    );
                    // Give a bit of time, this is a network error
                    // so if something ephemeral happened, it should be resolved by the time we try to reconnect
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    self.launch_client(ctrl_tx.clone()).await?;
                }
            }
        }
        Ok(())
    }
}

// Tests module
#[cfg(test)]
mod tests;
