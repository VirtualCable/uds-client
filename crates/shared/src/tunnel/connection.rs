#![allow(dead_code)] // TODO: remove soon :)
use std::sync::Arc;

use anyhow::{Context, Result};
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, RootCertStore};
use rustls_native_certs::load_native_certs;
use tokio::io::{ReadHalf, WriteHalf};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::{TlsConnector, client::TlsStream};

use super::consts;
use crate::tls::noverify;

pub async fn connect_and_upgrade(
    server: &str,
    port: u16,
    check_certificate: bool,
) -> Result<TlsStream<TcpStream>> {
    // Ensures TLS is initialized, with default ciphers right now

    let addr = format!("{}:{}", server, port);
    let mut tcp = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Failed to connect to {}", addr))?;

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

    Ok(tls_stream)
}

pub async fn send_cmd(
    reader: &mut ReadHalf<TlsStream<TcpStream>>,
    writer: &mut WriteHalf<TlsStream<TcpStream>>,
    cmd: &[u8],
) -> Result<()> {
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

pub async fn test_connection(
    reader: &mut ReadHalf<TlsStream<TcpStream>>,
    writer: &mut WriteHalf<TlsStream<TcpStream>>,
) -> Result<()> {
    send_cmd(reader, writer, consts::CMD_TEST).await
}

pub async fn open_connection(
    reader: &mut ReadHalf<TlsStream<TcpStream>>,
    writer: &mut WriteHalf<TlsStream<TcpStream>>,
    ticket: &str,
) -> Result<()> {
    // Convert ticket to bytes and send OPEN command
    let cmd_open = [consts::CMD_OPEN, ticket.as_bytes()].concat();
    send_cmd(reader, writer, &cmd_open).await
}
