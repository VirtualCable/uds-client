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

use anyhow::Result;
use rcgen::generate_simple_self_signed;
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

use rand::{Rng, distr::Alphanumeric, rng};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf, split},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tokio_rustls::{TlsAcceptor, client, server};

use super::{TunnelConnectInfo, consts, tunnel_runner};
use crate::system::trigger;
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

async fn echo_all_data(
    reader: &mut ReadHalf<server::TlsStream<TcpStream>>,
    writer: &mut WriteHalf<server::TlsStream<TcpStream>>,
    trigger: &Trigger,
) -> Result<()> {
    let mut buf = vec![0u8; consts::BUFFER_SIZE];

    loop {
        tokio::select! {
            res = reader.read(&mut buf) => {
                match res {
                    Ok(0) => {
                        // Peer closed; orderly shutdown our side
                        let _ = writer.shutdown().await;
                        break;
                    }
                    Ok(n) => {
                        let data = &buf[..n];
                        log::debug!("Echoing back {} bytes", n);
                        if let Err(e) = writer.write_all(data).await {
                            log::warn!("TLS write error: {:?}", e);
                            let _ = writer.shutdown().await;
                            break;
                        }
                    }
                    Err(e) => {
                        log::warn!("TLS read error: {:?}", e);
                        let _ = writer.shutdown().await;
                        break;
                    }
                }
            }
            _ = trigger.async_wait() => {
                // External cancellation: orderly shutdown
                let _ = writer.shutdown().await;
                break;
            }
        }
    }

    log::debug!("Echo task closed");
    Ok(())
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
    let tls_stream: server::TlsStream<TcpStream> = acceptor.accept(tcp).await?;

    // Step 3: command loop over TLS
    let (mut tls_reader, mut tls_writer) = split(tls_stream);
    let mut buf = vec![0u8; consts::BUFFER_SIZE];

    loop {
        tokio::select! {
            res = tls_reader.read(&mut buf) => {
                match res {
                    Ok(0) => {
                        // Peer closed; perform an orderly shutdown on our side
                        let _ = tls_writer.shutdown().await;
                        break;
                    }
                    Ok(n) => {
                        let data = &buf[..n];
                        log::debug!("Command received: {:?}", data);

                        let (start_echoing, resp): (bool, &[u8]) = if let Some(slice) = data.get(..consts::CMD_LENGTH) {
                            if slice == consts::CMD_TEST {
                                log::debug!("CMD_TEST received");
                                (false, consts::RESPONSE_OK)
                            } else if slice == consts::CMD_OPEN {
                                log::debug!("CMD_OPEN received");
                                // Read the ticket, for simplicity assume rest of data is ticket
                                let ticket = &data[consts::CMD_LENGTH..];
                                if ticket.len() != consts::TICKET_LENGTH {
                                    log::warn!("Invalid ticket length");
                                    break;
                                }
                                log::debug!("Ticket: {:?}", ticket);
                                (true, consts::RESPONSE_OK)
                            } else {
                                log::warn!("Unknown command: {:?}", slice);
                                (false, b"NOThe command is unknown")
                            }
                        } else {
                            log::warn!("Received data too short for command");
                            (false, b"NOThe command is too short")
                        };

                        if let Err(e) = tls_writer.write_all(resp).await {
                            log::warn!("TLS write error: {:?}", e);
                            let _ = tls_writer.shutdown().await;
                            break;
                        }

                        if start_echoing {
                            log::debug!("Starting to echo all data");
                            echo_all_data(&mut tls_reader, &mut tls_writer, &trigger).await?;
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
                // External cancellation: orderly shutdown
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

pub async fn connect(
    server: Option<&str>,
    port: u16,
) -> Result<(
    ReadHalf<client::TlsStream<TcpStream>>,
    WriteHalf<client::TlsStream<TcpStream>>,
    tokio::task::JoinHandle<()>,
    trigger::Trigger,
)> {
    log::setup_logging("debug", log::LogType::Tests);
    crate::tls::init_tls(None);
    let trigger = Trigger::new();
    let server_handle = if server.is_none() {
        log::debug!("Starting test server on port {}", port);
        tokio::spawn({
            let trigger = trigger.clone();
            async move {
                run_test_server(port, trigger).await.unwrap();
            }
        })
    } else {
        tokio::spawn(async {})
    };
    // Give the server a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let server = server.unwrap_or("localhost");
    log::debug!(
        "Starting client connection to test server: {}:{}",
        server,
        port
    );
    let (reader, writer) = super::connection::connect_and_upgrade(server, port, false)
        .await
        .expect("Failed to connect and upgrade to TLS");

    Ok((reader, writer, server_handle, trigger))
}

pub async fn create_runner(port: u16) -> Result<(JoinHandle<()>, JoinHandle<()>, u16, Trigger)> {
    log::setup_logging("debug", log::LogType::Tests);
    crate::tls::init_tls(None);

    let trigger = Trigger::new();

    let server_handle = tokio::spawn({
        let trigger = trigger.clone();
        async move {
            run_test_server(port, trigger).await.unwrap();
            log::debug!("Test server exited");
        }
    });

    let info = TunnelConnectInfo {
        addr: "localhost".to_string(),
        port,
        ticket: create_ticket(),
        local_port: None,
        check_certificate: false,
        startup_time_ms: 10000,
        keep_listening_after_timeout: false,
        enable_ipv6: false,
    };
    let listener = super::connection::create_listener(info.local_port, info.enable_ipv6)
        .await
        .unwrap();
    let listen_port = listener.local_addr()?.port();
    let runner_handle = tokio::spawn(async move {
        tunnel_runner(info, listener).await.unwrap();
    });
    // Shuld be running now, wait a moment
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    Ok((server_handle, runner_handle, listen_port, trigger))
}

pub fn create_ticket() -> String {
    rng()
        .sample_iter(&Alphanumeric)
        .take(consts::TICKET_LENGTH)
        .map(char::from)
        .collect::<String>()
}
