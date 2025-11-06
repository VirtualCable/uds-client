use std::io::Write;
use std::net::TcpStream;
use std::sync::Arc;

use anyhow::{Context, Result};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned, pki_types::ServerName};
use rustls_native_certs::load_native_certs;

use crate::tls::{init_tls, noverify};

use super::consts::HANDSHAKE_V1;

pub type TlsStream = StreamOwned<ClientConnection, TcpStream>;

#[allow(dead_code)]
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
