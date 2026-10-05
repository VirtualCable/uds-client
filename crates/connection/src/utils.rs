// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::{Context, Result};
use shared::log;

pub async fn create_listener(
    local_port: Option<u16>,
    enable_ipv6: bool,
) -> Result<tokio::net::TcpListener> {
    let addr = format!(
        "{}:{}",
        if enable_ipv6 {
            crate::consts::LISTEN_ADDRESS_V6
        } else {
            crate::consts::LISTEN_ADDRESS
        },
        local_port.unwrap_or(0)
    );
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("Failed to create TCP listener")?;

    log::debug!("TCP listener created on {}", addr);

    Ok(listener)
}

/// OS-level TCP keepalive backup for the tunnel connection.
///
/// The application-level keep-alive (`KEEPALIVE_INTERVAL_SECS` `Nop` frames)
/// is the primary liveness mechanism, but on a silently black-holed route the
/// TCP socket never errors and the local stack cannot tell the peer is gone.
/// SO_KEEPALIVE makes the kernel probe the peer so a dead leg is noticed by
/// both sides: the launcher's connect/recover loop then reacts to the socket
/// error instead of waiting for the server to reclaim the session.
///
/// Failures are non-fatal (best-effort); without them only the black-hole
/// case degrades to the application deadline alone.
pub fn set_keepalive(stream: &tokio::net::TcpStream) {
    // Must stay behind the application cadence: probe only legs that were
    // silent at the TCP layer for longer than the KA interval.
    const IDLE_SECS: u64 = 15;
    const INTERVAL_SECS: u64 = 5;
    const RETRIES: u32 = 3;

    let ka = socket2::TcpKeepalive::new()
        .with_time(std::time::Duration::from_secs(IDLE_SECS))
        .with_interval(std::time::Duration::from_secs(INTERVAL_SECS));
    let sock = socket2::SockRef::from(stream);
    #[cfg(unix)]
    let ka = ka.with_retries(RETRIES);

    if let Err(e) = sock.set_tcp_keepalive(&ka) {
        log::debug!("Failed to set TCP keepalive on tunnel connection: {:?}", e);
    }

    // TCP_USER_TIMEOUT is the option that actually bounds a *black-holed*
    // leg: the keep-alive frames written every 2s restart the SO_KEEPALIVE
    // idle timer, so a one-way data black hole would keep the socket
    // "alive" indefinitely. This option fails the socket once transmitted
    // data stays unacknowledged for the given time, regardless of how often
    // we write, which is what makes the connect/recover loop notice a dead
    // route. Linux-only; elsewhere SO_KEEPALIVE above is the best available
    // backup.
    #[cfg(target_os = "linux")]
    if let Err(e) = sock.set_tcp_user_timeout(Some(std::time::Duration::from_secs(
        IDLE_SECS + INTERVAL_SECS * RETRIES as u64,
    ))) {
        log::debug!(
            "Failed to set TCP_USER_TIMEOUT on tunnel connection: {:?}",
            e
        );
    }
}
