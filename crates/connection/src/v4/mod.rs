// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
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
use std::time::Duration;
use tokio::net::TcpListener;

mod connection;
mod consts;
mod proxy;

use crate::consts::MAX_STARTUP_TIME_MS;
use shared::log;

use crate::{registry, types::TunnelConnectInfo};

// On new releases, the min_listening_ms is the time the tunnel will stay alive waiting for initial connections
// on 4.0 and before, was the time that keeps the tunnel allowing new connnections (to disallow new connections after timeout)
// We hard limit this to max MAX_STARTUP_TIME_MS milliseconds to avoid very long living tunnels without connections, even in case of misconfiguration
pub async fn tunnel_runner(info: TunnelConnectInfo, listener: TcpListener) -> Result<()> {
    let (_id, trigger, active_connections) = registry::register_tunnel(Some(
        Duration::from_millis(info.startup_time_ms.min(MAX_STARTUP_TIME_MS)),
    ));

    loop {
        // Accept incoming connection until triggered to stop.
        tokio::select! {
            res = listener.accept() => {
                let (client_stream, client_addr) = res?;
                // Disable nagle's algorithm also on client side
                client_stream.set_nodelay(true).ok();

                log::debug!("Accepted connection from {}", client_addr);
                // Open connection, no new test is needed here since we already tested in start_tunnel
                let (mut reader, mut writer) = connection::connect_and_upgrade(
                    &info.addr,
                    info.port,
                    info.check_certificate,
                ).await?;
                connection::send_open_cmd(&mut reader, &mut writer, &info.ticket).await?;
                log::debug!("Tunnel connection established, starting proxying");
                // Start proxying in a new task
                tokio::spawn({
                    let trigger = trigger.clone();
                    let active_connections = active_connections.clone();
                    async move {
                        active_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if let Err(e) = proxy::start_proxy(
                            reader,
                            writer,
                            client_stream,
                            trigger,
                        ).await {
                            log::error!("Proxy error: {e}");
                        }
                        active_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                    }
                });
            }
            _ = trigger.wait_async() => {
                log::info!("Tunnel runner triggered to stop accepting new connections.");
                break;
            }
        }
    }

    log::debug!("Tunnel runner exiting");
    // Ensure our trigger is set
    trigger.trigger();

    Ok(())
}

pub async fn check_tunnel(info: &TunnelConnectInfo) -> Result<()> {
    let (mut reader, mut writer) =
        connection::connect_and_upgrade(&info.addr, info.port, info.check_certificate).await?;

    // Test to ensure connection is valid
    connection::send_test_cmd(&mut reader, &mut writer).await?;
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
