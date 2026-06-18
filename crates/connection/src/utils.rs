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
