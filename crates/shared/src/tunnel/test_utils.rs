use std::sync::Arc;

use anyhow::Result;
use rcgen::generate_simple_self_signed;
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::{TlsAcceptor, server::TlsStream};

use super::consts;
use crate::{log, system::trigger::Trigger};

/// Build an in-memory self-signed TLS ServerConfig suitable for tests.
fn build_test_tls_config() -> Arc<ServerConfig> {
    // Generate a self-signed cert + key
    let cert = generate_simple_self_signed(vec!["localhost".into()]).unwrap();

    // Cert and key
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let pkcs8 = PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
    let key_der = PrivateKeyDer::from(pkcs8);

    // Server config
    Arc::new(
        ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .expect("Failed to create ServerConfig"),
    )
}

/// clear Handshake + upgrade TLS + command loop
async fn handle_client(mut tcp: TcpStream, acceptor: TlsAcceptor, trigger: Trigger) -> Result<()> {
    // handshake in plain TCP
    let mut buf = vec![0u8; consts::HANDSHAKE_V1.len()];
    tcp.read_exact(&mut buf).await?;
    if buf != consts::HANDSHAKE_V1 {
        log::warn!("Invalid handshake: {:?}", &buf);
        return Ok(());
    }
    log::debug!("Handshake received correctly");

    // upgrade to TLS
    // Note: TlsAcceptor wraps the creation of ServerConnection and async handshake
    let tls_stream: TlsStream<TcpStream> = acceptor.accept(tcp).await?;

    // Step 3: command loop over TLS
    let (mut tls_reader, mut tls_writer) = tokio::io::split(tls_stream);
    let mut buf = vec![0u8; consts::BUFFER_SIZE];

    loop {
        tokio::select! {
            res = tls_reader.read(&mut buf) => {
                match res {
                    Ok(0) => {
                        // Peer cerró; cierre ordenado nuestro lado
                        let _ = tls_writer.shutdown().await;
                        break;
                    }
                    Ok(n) => {
                        let data = &buf[..n];
                        log::debug!("Command received: {:?}", data);

                        let resp = if let Some(slice) = data.get(..consts::CMD_LENGTH) {
                            if slice == consts::CMD_TEST {
                                log::debug!("CMD_TEST received");
                                consts::RESPONSE_OK
                            } else if slice == consts::CMD_OPEN {
                                log::debug!("CMD_OPEN received");
                                // Read the ticket, for simplicity assume rest of data is ticket
                                let ticket = &data[consts::CMD_LENGTH..];
                                if ticket.len() != consts::TICKET_LENGTH {
                                    log::warn!("Invalid ticket length");
                                    break;
                                }
                                log::debug!("Ticket: {:?}", ticket);
                                consts::RESPONSE_OK
                            } else {
                                log::warn!("Unknown command: {:?}", slice);
                                b"NOThe command is unknown"
                            }
                        } else {
                            log::warn!("Received data too short for command");
                            b"NOThe command is too short"
                        };

                        if let Err(e) = tls_writer.write_all(resp).await {
                            log::warn!("TLS write error: {:?}", e);
                            let _ = tls_writer.shutdown().await;
                            break;
                        }
                    }
                    Err(e) => {
                        log::warn!("TLS read error: {:?}", e);
                        let _ = tls_writer.shutdown().await;
                        break;
                    }
                }
            }
            _ = trigger.async_wait() => {
                // Cancelación externa: cierre ordenado
                let _ = tls_writer.shutdown().await;
                break;
            }
        }
    }

    log::debug!("Client closed");
    Ok(())
}

/// Test server async: accepts connections, upgrades to TLS, and handles commands.
pub async fn run_test_server(port: u16, trigger: Trigger) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await?;
    log::info!("Test server listening on {}", addr);

    let config = build_test_tls_config();
    let acceptor = TlsAcceptor::from(config);

    loop {
        tokio::select! {
            res = listener.accept() => {
                match res {
                    Ok((stream, _peer)) => {
                        let acceptor = acceptor.clone();
                        let trig = trigger.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_client(stream, acceptor, trig).await {
                                log::warn!("Client handling error: {:?}", e);
                            }
                        });
                    }
                    Err(e) => {
                        log::warn!("Accept error: {:?}", e);
                        continue;
                    }
                }
            }
            _ = trigger.async_wait() => {
                log::info!("Test server stopped by trigger");
                break;
            }
        }
    }

    Ok(())
}
