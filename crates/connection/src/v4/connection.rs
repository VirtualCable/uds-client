// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::Arc;

use anyhow::{Context, Result};
use crypt::types::Ticket;
use rustls::{ClientConfig, pki_types::ServerName};
// Only the secure branch (release without `insecure-tls`) loads native certs
#[cfg(not(any(debug_assertions, feature = "insecure-tls")))]
use rustls::RootCertStore;
#[cfg(not(any(debug_assertions, feature = "insecure-tls")))]
use rustls_native_certs::load_native_certs;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf, split},
    net::TcpStream,
    time::timeout,
};
use tokio_rustls::{TlsConnector, client::TlsStream};

use super::consts;
use crate::tls_error;
use shared::log;
// The bypass machinery only exists in insecure builds
#[cfg(any(debug_assertions, feature = "insecure-tls"))]
use shared::tls::noverify;

pub async fn connect_and_upgrade(
    server: &str,
    port: u16,
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

    // TLS verification policy is decided at COMPILE time, not by callers:
    //
    // * Insecure builds (debug or `--features insecure-tls`) never verify
    //   certificates, and the launcher shows a blocking warning about it at
    //   startup. Only such builds contain the `noverify` machinery at all.
    // * Secure release builds do not even compile the bypass branch: there is
    //   no runtime flag (config, app_data.json, script parameters) that can
    //   turn verification off. See `shared::tls::insecure_tls_bypass_allowed`.
    #[cfg(any(debug_assertions, feature = "insecure-tls"))]
    let config: Arc<ClientConfig> = noverify::client_config();

    #[cfg(not(any(debug_assertions, feature = "insecure-tls")))]
    let config: Arc<ClientConfig> = {
        let mut root_store = RootCertStore::empty();
        let certs_result = load_native_certs();

        if !certs_result.errors.is_empty() {
            for err in certs_result.errors {
                log::warn!("Failed to load a native certificate: {}", err);
            }
        }

        for cert in certs_result.certs {
            root_store.add(cert).unwrap_or_else(|e| {
                log::warn!("Failed to add a native certificate to root store: {:?}", e);
            });
        }

        Arc::new(
            ClientConfig::builder()
                .with_root_certificates(Arc::new(root_store))
                .with_no_client_auth(),
        )
    };

    let connector = TlsConnector::from(config);

    // Perform TLS handshake
    let server_name =
        ServerName::try_from(server.to_string()).context("Invalid server name for TLS")?;
    let endpoint = format!("{server}:{port}");
    let tls_stream = connector
        .connect(server_name, tcp)
        .await
        .map_err(|e| anyhow::anyhow!(handshake_error_message(&e, &endpoint)))?;

    Ok(split(tls_stream))
}

/// Walks `err` looking for the deepest TLS-level source and returns an
/// actionable message that names `endpoint`. Exposed for tests so the
/// mapping can be exercised without a live TLS handshake.
fn handshake_error_message(err: &(dyn std::error::Error + 'static), endpoint: &str) -> String {
    let mut cur: &dyn std::error::Error = err;
    let mut raw = cur.to_string();
    while let Some(next) = cur.source() {
        cur = next;
        let s = cur.to_string();
        if tls_error::looks_like_tls_error(&s) {
            raw = s;
            break;
        }
    }
    tls_error::classify(&raw, endpoint)
}

async fn send_cmd<R, W>(reader: &mut R, writer: &mut W, cmd: &[u8]) -> Result<()>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
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

pub async fn send_test_cmd<R, W>(reader: &mut R, writer: &mut W) -> Result<()>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    // Send CMD_TEST with timeout
    timeout(
        consts::CMD_TIMEOUT_SECS,
        send_cmd(reader, writer, consts::CMD_TEST),
    )
    .await
    .context("CMD_TEST timed out")?
}

pub async fn send_open_cmd<R, W>(reader: &mut R, writer: &mut W, ticket: &Ticket) -> Result<()>
where
    R: AsyncReadExt + Unpin,
    W: AsyncWriteExt + Unpin,
{
    // Convert ticket to bytes and send OPEN command with timeout

    let cmd_open = [consts::CMD_OPEN, ticket.as_ref()].concat();
    timeout(
        consts::CMD_TIMEOUT_SECS,
        send_cmd(reader, writer, &cmd_open),
    )
    .await
    .context("CMD_OPEN timed out")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as StdError;

    #[derive(Debug)]
    struct Layer {
        msg: String,
        source: Option<Box<dyn StdError + Send + Sync + 'static>>,
    }

    impl std::fmt::Display for Layer {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.msg)
        }
    }

    impl StdError for Layer {
        fn source(&self) -> Option<&(dyn StdError + 'static)> {
            self.source
                .as_deref()
                .map(|e| e as &(dyn StdError + 'static))
        }
    }

    fn layer(msg: &str) -> Layer {
        Layer {
            msg: msg.into(),
            source: None,
        }
    }

    fn layer_above(msg: &str, below: Layer) -> Layer {
        Layer {
            msg: msg.into(),
            source: Some(Box::new(below)),
        }
    }

    #[test]
    fn handshake_error_message_digs_into_source_chain_for_tls_variant() {
        // Mimic tokio_rustls wrapping rustls::Error: top says "transport",
        // second says "io error", third (the rustls one) names the variant.
        let err = layer_above(
            "transport error",
            layer_above(
                "io error",
                layer("invalid peer certificate: NotValidForName"),
            ),
        );

        let msg = handshake_error_message(&err, "broker.example.com:15443");
        assert!(msg.starts_with("TLS: "), "got: {msg}");
        assert!(msg.contains("Subject Alternative Name"), "got: {msg}");
        assert!(msg.contains("broker.example.com:15443"), "got: {msg}");
        assert!(!msg.contains("NotValidForName"));
    }

    #[test]
    fn handshake_error_message_uses_top_message_when_no_tls_in_chain() {
        let err = layer("connection refused");
        let msg = handshake_error_message(&err, "broker.example.com:15443");
        // No TLS signal anywhere: the raw message is returned verbatim by
        // the classifier, with no host suffix or prefix.
        assert_eq!(msg, "connection refused");
    }

    #[test]
    fn handshake_error_message_includes_endpoint_for_unknown_tls() {
        let err = layer("invalid peer certificate: SomeFutureVariant");
        let msg = handshake_error_message(&err, "192.168.15.69:15443");
        assert!(msg.starts_with("TLS: "), "got: {msg}");
        assert!(msg.contains("SomeFutureVariant"), "got: {msg}");
        assert!(msg.contains("192.168.15.69:15443"), "got: {msg}");
    }
}
