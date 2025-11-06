use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rustls::{ServerConfig, StreamOwned};
use rustls::server::ServerConnection;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rcgen::generate_simple_self_signed;

use super::consts::*;
use crate::{system::trigger::Trigger, log};

/// Build an in-memory self-signed TLS ServerConfig suitable for tests.
fn build_test_tls_config() -> Arc<ServerConfig> {
    // Generate a self-signed cert + key
    let cert = generate_simple_self_signed(vec!["localhost".into()]).unwrap();

    // Access certificate and signing key
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    // rcgen returns a PKCS#8 DER-encoded private key; wrap it as a PKCS#8 key and then convert to PrivateKeyDer
    let pkcs8 = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
    let key_der = PrivateKeyDer::from(pkcs8);

    // Build server config
    let server_config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("Failed to create ServerConfig");

    Arc::new(server_config)
}


/// Handle a single client: clear handshake, TLS upgrade, command loop over TLS.
fn handle_client(mut tcp: TcpStream, config: Arc<ServerConfig>, trigger: Trigger) {
    // Handshake
    let mut buf = vec![0u8; HANDSHAKE_V1.len()];
    match tcp.read(&mut buf) {
        Ok(n) if &buf[..n] == HANDSHAKE_V1 => {
            log::debug!("Handshake received correctly");
        }
        Ok(n) => {
            log::warn!("Invalid handshake: {:?}", &buf[..n]);
            return;
        }
        Err(e) => {
            log::warn!("Handshake read error: {:?}", e);
            return;
        }
    }

    // Configure read timeout so read() returns periodically (to poll the trigger)
    tcp.set_read_timeout(Some(Duration::from_secs(1))).unwrap();

    // Upgrade to TLS
    let conn = ServerConnection::new(config).unwrap();
    let mut tls = StreamOwned::new(conn, tcp);

    // Command loop over TLS (non-blocking via timeout)
    let mut buf = vec![0u8; BUFFER_SIZE];
    loop {
        if trigger.is_set() {
            break;
        }

        match tls.read(&mut buf) {
            Ok(0) => break, // connection closed
            Ok(n) => {
                let data = &buf[..n];
                log::debug!("Command received: {:?}", data);

                if data == CMD_TEST || data == CMD_OPEN {
                    let _ = tls.write_all(RESPONSE_OK);
                } else {
                    let _ = tls.write_all(b"ERR");
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data available now; continue to poll trigger
                continue;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Timeout: opportunity to poll trigger; keep looping
                continue;
            }
            Err(e) => {
                log::warn!("TLS read error: {:?}", e);
                break;
            }
        }
    }

    log::debug!("Client closed");
}

/// Test server: accepts non-blocking, spawns a thread per client, stops on trigger.
pub fn run_test_server(port: u16, trigger: Trigger) {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).expect("Failed to bind test server");
    listener.set_nonblocking(true).unwrap();
    log::info!("Test server listening on {}", addr);

    let config = build_test_tls_config();

    while !trigger.is_set() {
        match listener.accept() {
            Ok((stream, _)) => {
                let cfg = config.clone();
                let trig = trigger.clone();
                thread::spawn(move || handle_client(stream, cfg, trig));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No pending connections; keep polling trigger
                continue;
            }
            Err(e) => {
                log::warn!("Accept error: {:?}", e);
                break;
            }
        }
    }

    log::info!("Test server stopped by trigger");
}
