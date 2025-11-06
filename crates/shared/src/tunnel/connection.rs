#![allow(dead_code)]
use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned, pki_types::ServerName};
use rustls_native_certs::load_native_certs;

use crate::{
    system::trigger::Trigger,
    tls::{init_tls, noverify},
};

use super::consts::HANDSHAKE_V1;

pub type TlsStream = StreamOwned<ClientConnection, TcpStream>;

pub fn connect_and_upgrade(
    server: &str,
    port: u16,
    check_certificate: bool,
    ciphers: Option<&str>,
) -> Result<TlsStream> {
    // Ensure TLS provider is initialized once
    init_tls(ciphers);

    let addr = format!("{}:{}", server, port);
    let mut tcp =
        TcpStream::connect(&addr).with_context(|| format!("Failed to connect to {}", addr))?;
    tcp.set_nodelay(true).ok();
    // Set small timeout on read, to avoid blocking as much as possible
    tcp.set_read_timeout(Some(std::time::Duration::from_millis(500)))
        .ok();

    // Step 1: Send handshake in plain TCP
    tcp.write_all(HANDSHAKE_V1)
        .context("Failed to send HANDSHAKE_V1")?;
    tcp.flush().ok();

    // Step 2: Build ClientConfig
    let config: Arc<ClientConfig> = if check_certificate {
        // Load native certs and build RootCertStore
        let mut root_store = RootCertStore::empty();
        let certs_result = load_native_certs();

        // If any error Ocurs, cert_rsult will have the errors array filled
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
        // Use your NoVerifySsl
        noverify::client_config()
    };

    // Step 3: Upgrade to TLS
    let server_name =
        ServerName::try_from(server.to_string()).context("Invalid server name for TLS")?;
    let conn = ClientConnection::new(config, server_name).context("TLS handshake failed")?;
    let tls = StreamOwned::new(conn, tcp);

    Ok(tls)
}

pub fn read_timeout(
    tls_stream: &mut TlsStream,
    buf: &mut [u8],
    timeout: Duration,
    trigger: Trigger,
) -> Result<Option<usize>> {
    let start = std::time::Instant::now();
    loop {
        // Note: We already have set the timeout on the underlying TcpStream (on connect_and_upgrade)
        // To allow
        if start.elapsed() >= timeout {
            return Ok(None);  // timeout
        }
        // Lets see if the trigger is set
        if trigger.is_set() {
            return Err(anyhow::anyhow!("Read aborted by trigger"));
        }
        // read should block for a while, because we are in blocking mode with a timeout
        // So a busy loop is avoided
        match tls_stream.read(buf) {
            Ok(n) => return Ok(Some(n)),  // 0 means connection closed
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available now; continue to poll trigger
                continue;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // No data available now; continue to poll trigger
                continue;
            }
            Err(e) => return Err(anyhow::anyhow!("Read error: {}", e)),
        }
    }
}

pub fn read_exact_timeout(
    tls_stream: &mut TlsStream,
    buf: &mut [u8],
    timeout: Duration,
    trigger: Trigger,
) -> Result<usize> {
    let mut total_read = 0;
    let deadline = Instant::now() + timeout;
    while total_read < buf.len() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(total_read); // Timeout global alcanzado
        }        
        let n = read_timeout(
            tls_stream,
            &mut buf[total_read..],
            remaining,
            trigger.clone(),
        )?;
        match n {
            Some(0) => return Err(anyhow::anyhow!("Connection closed before reading enough data")),
            Some(n) => total_read += n,
            None => return Ok(total_read),  // Timeout, return exactly what we have read so far
        }
    }
    Ok(total_read)
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::run_test_server;
    use super::*;
    use crate::log;
    use crate::system::trigger::Trigger;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_connect_and_upgrade() {
        log::setup_logging("debug", log::LogType::Tests);
        crate::tls::init_tls(None);
        let trigger = Trigger::new();
        let server_handle = thread::spawn({
            let trigger = trigger.clone();
            move || {
                run_test_server(44910, trigger);
            }
        });
        // Give the server a moment to start
        thread::sleep(Duration::from_millis(500));
        let tls_stream = connect_and_upgrade("localhost", 44910, false, None)
            .expect("Failed to connect and upgrade to TLS");
        // If we reach here, the connection and upgrade were successful
        drop(tls_stream);
        trigger.set();
        server_handle.join().unwrap();
    }
}
