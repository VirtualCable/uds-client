// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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
use std::sync::Arc;

use anyhow::{Context, Result};
use rustls::{
    pki_types::ServerName,
    {ClientConfig, RootCertStore},
};
use rustls_native_certs::load_native_certs;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf, split},
    net::TcpStream,
    time::timeout,
};
use tokio_rustls::{TlsConnector, client::TlsStream};

use super::consts;
use crate::{log, tls::noverify};

pub async fn connect_and_upgrade(
    server: &str,
    port: u16,
    check_certificate: bool,
) -> Result<(
    ReadHalf<TlsStream<TcpStream>>,
    WriteHalf<TlsStream<TcpStream>>,
)> {
    // Ensures TLS is initialized, with default ciphers right now

    let addr = format!("{}:{}", server, port);
    let mut tcp = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Failed to connect to {}", addr))?;

    // Disable nagle's algorithm
    tcp.set_nodelay(true).ok();

    // Send handshake pre-TLS
    tcp.write_all(consts::HANDSHAKE_V1)
        .await
        .context("Failed to send HANDSHAKE_V1")?;
    tcp.flush().await.ok();

    // Build TLS client config
    let config: Arc<ClientConfig> = if check_certificate {
        let mut root_store = RootCertStore::empty();
        let certs_result = load_native_certs();

        if !certs_result.errors.is_empty() {
            for err in certs_result.errors {
                crate::log::warn!("Failed to load a native certificate: {}", err);
            }
        }

        for cert in certs_result.certs {
            root_store.add(cert).unwrap_or_else(|e| {
                crate::log::warn!("Failed to add a native certificate to root store: {:?}", e);
            });
        }

        Arc::new(
            ClientConfig::builder()
                .with_root_certificates(Arc::new(root_store))
                .with_no_client_auth(),
        )
    } else {
        noverify::client_config()
    };

    let connector = TlsConnector::from(config);

    // Perform TLS handshake
    let server_name =
        ServerName::try_from(server.to_string()).context("Invalid server name for TLS")?;
    let tls_stream = connector
        .connect(server_name, tcp)
        .await
        .context("TLS handshake failed")?;

    Ok(split(tls_stream))
}

async fn send_cmd(
    reader: &mut ReadHalf<TlsStream<TcpStream>>,
    writer: &mut WriteHalf<TlsStream<TcpStream>>,
    cmd: &[u8],
) -> Result<()> {
    log::debug!("Sending command: {:?}", cmd);
    // Send command
    writer
        .write_all(cmd)
        .await
        .context("Failed to send command")?;
    writer.flush().await.ok();
    // Expect OK response
    let mut buf = vec![0u8; consts::RESPONSE_OK.len()];
    reader
        .read_exact(&mut buf)
        .await
        .context("Failed to read command response")?;
    if buf != consts::RESPONSE_OK {
        return Err(anyhow::anyhow!("Invalid command response: {:?}", buf));
    }

    Ok(())
}

pub async fn send_test_cmd(
    reader: &mut ReadHalf<TlsStream<TcpStream>>,
    writer: &mut WriteHalf<TlsStream<TcpStream>>,
) -> Result<()> {
    // Send CMD_TEST with timeout
    timeout(
        consts::CMD_TIMEOUT_SECS,
        send_cmd(reader, writer, consts::CMD_TEST),
    )
    .await
    .context("CMD_TEST timed out")?
}

pub async fn send_open_cmd(
    reader: &mut ReadHalf<TlsStream<TcpStream>>,
    writer: &mut WriteHalf<TlsStream<TcpStream>>,
    ticket: &str,
) -> Result<()> {
    // Convert ticket to bytes and send OPEN command with timeout

    let cmd_open = [consts::CMD_OPEN, ticket.as_bytes()].concat();
    timeout(
        consts::CMD_TIMEOUT_SECS,
        send_cmd(reader, writer, &cmd_open),
    )
    .await
    .context("CMD_OPEN timed out")?
}

pub async fn create_listener(
    local_port: Option<u16>,
    enable_ipv6: bool,
) -> Result<tokio::net::TcpListener> {
    let addr = format!(
        "{}:{}",
        if enable_ipv6 {
            consts::LISTEN_ADDRESS_V6
        } else {
            consts::LISTEN_ADDRESS
        },
        local_port.unwrap_or(0)
    );
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .context("Failed to create TCP listener")?;

    log::debug!("TCP listener created on {}", addr);

    Ok(listener)
}
