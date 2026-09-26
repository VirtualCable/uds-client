// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::{Ok, Result};
use crypt::secrets::derive_tunnel_material;
use std::time::Duration;
use {
    tokio::io::{AsyncReadExt, AsyncWriteExt},
    tokio::net::TcpListener,
};

use shared::log;

use crate::{consts::MAX_STARTUP_TIME_MS, registry, types::TunnelConnectInfo};

pub mod client;
pub mod protocol;
pub mod proxy;
pub mod server;
pub mod udp;

use protocol::consts::HANDSHAKE_TEST_RESPONSE;

/// Tries to start the UDP leg of a tunnel session. Any failure degrades to
/// TCP-only (logged, never breaks the session).
///
/// - Local listen port: `info.udp_port` if set, otherwise the TCP listener
///   port (mstsc expects UDP on the same port it connected to over TCP).
/// - Remote port: the `udp_port` reported by the server in the open response
///   if non-zero, otherwise the TCP tunnel port.
async fn try_start_udp_relay(
    info: &TunnelConnectInfo,
    shared_secret: &crypt::types::SharedSecret,
    token: crypt::datagram::UdpToken,
    server_udp_port: u16,
    tcp_listener_port: u16,
    stop: shared::system::trigger::Trigger,
) -> Option<udp::UdpRelay> {
    let local_port = info.udp_port.unwrap_or(tcp_listener_port);
    let remote_port = if server_udp_port != 0 {
        server_udp_port
    } else {
        info.port
    };
    let start = async {
        let (inbound, outbound) = crypt::secrets::get_udp_crypts(shared_secret, &info.ticket)?;
        udp::UdpRelay::start(
            local_port,
            info.enable_ipv6,
            &info.addr,
            remote_port,
            token,
            inbound,
            outbound,
            stop,
        )
        .await
    };
    match start.await {
        std::result::Result::Ok(relay) => {
            log::info!(
                "UDP relay listening on {}, forwarding to {}:{}",
                relay.local_addr(),
                info.addr,
                remote_port
            );
            Some(relay)
        }
        Err(e) => {
            log::error!("UDP relay unavailable, continuing TCP-only: {e}");
            None
        }
    }
}

pub async fn tunnel_runner(info: TunnelConnectInfo, listener: TcpListener) -> Result<()> {
    log::debug!(
        "Starting tunnel runner with startup_time_ms: {}, max allowed: {}",
        info.startup_time_ms,
        MAX_STARTUP_TIME_MS
    );
    let (_id, registered_trigger, active_connections) = registry::register_tunnel(Some(
        Duration::from_millis(info.startup_time_ms.min(MAX_STARTUP_TIME_MS)),
    ));
    let shared_secret = info.shared_secret.clone().ok_or(anyhow::format_err!(
        "TunnelConnectInfo must include shared secret"
    ))?;

    // Derive tunnel material for decryption of data
    let crypt_info = derive_tunnel_material(&shared_secret, &info.ticket)?;

    loop {
        // Accept incoming connection until triggered to stop.
        tokio::select! {
            res = listener.accept() => {
                let (client_stream, client_addr) = res?;
                // Disable nagle's algorithm also on client side
                client_stream.set_nodelay(true).ok();

                log::debug!("Accepted connection from {}", client_addr);

                // Launch the proxy, register a client and launc it.
                // We will wait for client to end for cleanup
                // Currently, as only one channel is being used
                // this will be enough
                let proxy = proxy::Proxy::new(
                    &format!("{}:{}", info.addr, info.port),
                    info.ticket.clone(),
                    crypt_info.clone(),
                    std::time::Duration::from_millis(info.startup_time_ms.min(MAX_STARTUP_TIME_MS)),
                    registered_trigger.clone(),
                );
                let udp_token_handle = proxy.udp_token_handle();
                let proxy = proxy.run().await?;

                let (reader, writer) = client_stream.into_split();

                let channels = proxy.request_channel(1).await?;

                // UDP leg: only if requested and the server assigned a
                // non-zero token in the open response. Failures degrade to
                // TCP-only, never break the session.
                let udp_relay = if info.use_udp {
                    let (token, server_udp_port) =
                        udp_token_handle.lock().unwrap().unwrap_or(([0u8; 16], 0));
                    if token == [0u8; 16] {
                        log::debug!("UDP requested but not enabled by server (zero token)");
                        None
                    } else {
                        try_start_udp_relay(
                            &info,
                            &shared_secret,
                            token,
                            server_udp_port,
                            listener.local_addr()?.port(),
                            registered_trigger.clone(),
                        )
                        .await
                    }
                } else {
                    None
                };

                let server = server::TunnelServer::new(
                    reader,
                    writer,
                    1,
                    channels.tx.clone(),
                    channels.rx.clone(),
                    registered_trigger.clone(),
                    proxy.clone(),
                );

                log::debug!("Tunnel connection established, starting proxying");
                // Start proxying in a new task
                tokio::spawn({
                    let active_connections = active_connections.clone();
                    // let registered_trigger = registered_trigger.clone();
                    async move {
                        // The UDP relay dies with this TCP connection
                        let _udp_relay = udp_relay;
                        active_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        log::debug!("Spawning tunnel server task, active connections: {}", active_connections.load(std::sync::atomic::Ordering::Relaxed));
                        if let Err(e) = server.run().await {
                            log::error!("Tunnel server error: {:?}", e.to_string());
                        } else {
                            log::debug!("Tunnel server exited normally");
                            // Delay a bit to conclude proxy, client, etc..
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                        log::debug!("Tunnel server task ended, active connections: {}", active_connections.load(std::sync::atomic::Ordering::Relaxed));
                        active_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        // proxy.release_channel(1).await.ok();
                        log::debug!("Tunnel connection closed, active connections: {}", active_connections.load(std::sync::atomic::Ordering::Relaxed));
                        // Ensure our proxy is stopped
                        // registered_trigger.trigger();
                    }
                });
            }
            _ = registered_trigger.wait_async() => {
                log::info!("Tunnel runner triggered to stop accepting new connections.");
                break;
            }
        }
    }

    log::debug!("Tunnel runner exiting");
    // Ensure our trigger is set
    registered_trigger.trigger();

    Ok(())
}

pub async fn check_tunnel(info: &TunnelConnectInfo) -> Result<()> {
    let remote_server_addr = format!("{}:{}", info.addr, info.port);
    let mut stream = tokio::net::TcpStream::connect(&remote_server_addr).await?;
    // Send Test Handshake
    let data = protocol::handshake::Handshake::Test.to_bytes();
    stream.write_all(&data).await?;
    // Read response, should be OK
    let mut buf = [0u8; 2];
    stream.read_exact(&mut buf).await?;
    if buf != *HANDSHAKE_TEST_RESPONSE {
        anyhow::bail!(
            "Unexpected handshake test response: {:?}, expected: {:?}",
            buf,
            HANDSHAKE_TEST_RESPONSE
        );
    }
    Ok(())
}

pub async fn start_tunnel(info: TunnelConnectInfo) -> Result<u16> {
    // This works this way:
    // 0. Connect to remote server and upgrade to TLS, test connection and close initial connection. (for early failure detection)
    // 1. Listen to local port (info.local_port or random)
    // 2. On connection, connect to remote server and upgrade to TLS
    // 3. Open
    // 3. Start proxying data between local port and TLS connection

    log::debug!("Creating local listener");
    // Open listener here to get the actual port, but move the listener into the tunnel runner
    let listener = crate::utils::create_listener(info.local_port, info.enable_ipv6).await?;
    let actual_port = listener.local_addr()?.port();

    log::info!(
        "Tunnel listening on port {}, forwarding to {}:{}",
        actual_port,
        info.addr,
        info.port
    );
    tokio::spawn({
        async move {
            if let Err(e) = tunnel_runner(info, listener).await {
                log::error!("Tunnel error: {e}");
            }
        }
    });

    Ok(actual_port)
}

#[cfg(test)]
mod tests;
