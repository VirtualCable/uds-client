// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
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
use std::time::Duration;

use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpListener};

use shared::log;

use crypt::{
    secrets::{CryptoKeys, derive_tunnel_material, get_tunnel_crypts},
    tunnel::types::PacketBuffer,
    types::{SharedSecret, Ticket},
};

use super::super::{
    protocol::{consts::HANDSHAKE_V2_SIGNATURE, payload_pair, payload_with_channel_pair},
    proxy::handler::ServerChannels,
};

use super::*;

// Helper to create dummy ticket, ensure always the same
fn dummy_ticket() -> Ticket {
    Ticket::new([b'x'; 48])
}

fn dummy_shared_secret() -> SharedSecret {
    SharedSecret::new([0u8; 32])
}

// Helper to create dummy shared secret
fn dummy_crypt_info() -> CryptoKeys {
    derive_tunnel_material(&dummy_shared_secret(), &dummy_ticket()).unwrap()
}

struct RemoteServer {
    addr: String,
    stop: Trigger,
    rx: PayloadWithChannelReceiver,
    tx: PayloadWithChannelSender,
}

async fn remote_server_dispatcher(
    stop: Trigger,
    mut socket: tokio::net::TcpStream,
    rx: PayloadWithChannelReceiver,
    tx: PayloadWithChannelSender,
) {
    let (mut crypt_output, mut crypt_input) =
        get_tunnel_crypts(&dummy_crypt_info(), (0, 0)).unwrap();

    let ticket = dummy_ticket();
    let mut buf = PacketBuffer::new();
    // Read handshake, but do not check it, just skip for tests
    // Do not check real data received, that has it specific test elsewhere
    {
        let handshake_buf = &mut [0u8; HANDSHAKE_V2_SIGNATURE.len() + 1 + 48]; // Handshake header + cmd + ticket
        socket.read_exact(handshake_buf).await.unwrap();
        // Now read encripted ticket again, but do not check it, just skip for tests
        crypt_input
            .read(&stop, &mut socket, &mut buf)
            .await
            .unwrap();
    }
    if let Err(e) = {
        crypt_output
            .write(&stop, &mut socket, 0, ticket.as_ref())
            .await
    } {
        log::error!("Error writing to socket: {:?}", e);
        stop.trigger();
        return;
    }

    loop {
        tokio::select! {
            _ = stop.wait_async() => {
                log::debug!("Stop signal received, shutting down remote server dispatcher");
                return;
            }
            data = crypt_input.read(&stop, &mut socket, &mut buf) => {
                log::debug!("Data received: {:?}", data);
                // Decrypt data
                let (data, channel_id) = match data {
                    Ok((data, channel_id)) => (data, channel_id),
                    Err(e) => {
                        log::error!("Error reading from socket: {:?}", e);
                        stop.trigger();
                        return;
                    }
                };
                // Send data back, same as received and to tx
                if let Err(e) = tx.send_async(PayloadWithChannel::new(channel_id, data)).await {
                    log::error!("Error sending to channel: {:?}", e);
                    stop.trigger();
                    return;
                }
            }
            channel_data = rx.recv_async() => {
                log::debug!("Data received from channel: {:?}", channel_data);
                match channel_data {
                    Ok(data) => {
                        log::debug!("Data received from channel: {:?}", data);
                        crypt_output.write(&stop, &mut socket, data.channel_id, &data.payload).await.unwrap_or_else(|e| {
                            log::error!("Error writing to socket: {:?}", e);
                            stop.trigger();
                        });
                    },
                    Err(e) => {
                        log::error!("Error receiving from channel: {:?}", e);
                        stop.trigger();
                        return;
                    }
                }
            }
        }
    }
}

async fn dummy_remote_server() -> RemoteServer {
    let stop = Trigger::new();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let (tx, server_rx) = flume::unbounded();
    let (server_tx, rx) = flume::unbounded();

    tokio::spawn({
        let stop = stop.clone();
        async move {
            tokio::select! {

                _ = stop.wait_async() => {
                    log::debug!("Stop signal received, shutting down dummy remote server");
                }
                accepted = listener.accept() => {
                    match accepted {
                        Ok((socket, _)) => {
                            log::debug!("Client connected to dummy remote server");
                            socket.set_nodelay(true).unwrap_or_else(|e| {
                                log::error!("Error setting nodelay on socket: {:?}", e);
                            });
                            remote_server_dispatcher(stop, socket, server_rx, server_tx).await;
                        }
                        Err(e) => {
                            log::error!("Error accepting connection: {:?}", e);
                            stop.trigger();
                        }
                    }
                }
            }
        }
    });

    log::debug!("Dummy remote server listening on {}", address);

    RemoteServer {
        addr: address,
        stop,
        rx,
        tx,
    }
}

#[tokio::test]
async fn test_stop_signal() -> Result<()> {
    log::setup_logging("debug", log::LogType::Test);

    let remote_server = dummy_remote_server().await;
    let stop = Trigger::new();
    let proxy = Proxy::new(
        &remote_server.addr,
        dummy_ticket(),
        dummy_crypt_info(),
        Duration::from_millis(100),
        stop.clone(),
    );

    let (ctrl_tx, ctrl_rx) = Handler::new_command_channel();

    let stopped = Trigger::new();
    tokio::spawn({
        let stopped = stopped.clone();
        async move {
            if let Err(e) = proxy.run_task(ctrl_tx, ctrl_rx).await {
                log::error!("Proxy run_task error: {:?}", e);
            } else {
                stopped.trigger();
            }
        }
    });

    stop.trigger();
    stopped
        .wait_timeout_async(std::time::Duration::from_secs(1))
        .await
        .context("Proxy did not stop within timeout")?;
    Ok(())
}

#[tokio::test]
async fn test_proxy_connection_fail() {
    log::setup_logging("debug", log::LogType::Test);
    // Bind to port 0 to get a free port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    // Drop listener to close the port
    drop(listener);

    let proxy = Proxy::new(
        &addr.to_string(),
        dummy_ticket(),
        dummy_crypt_info(),
        Duration::from_millis(100),
        Trigger::new(),
    );

    // Should fail to connect
    let result = proxy.run().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_proxy_handshake_fail_garbage() {
    log::setup_logging("debug", log::LogType::Test);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn a dummy server that sends garbage
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let _ = socket.write_all(b"garbage data").await;
        }
    });

    let proxy = Proxy::new(
        &addr.to_string(),
        dummy_ticket(),
        dummy_crypt_info(),
        Duration::from_millis(100),
        Trigger::new(),
    );

    // Should fail during handshake
    let result = proxy.run().await;
    assert!(result.is_err());
    log::debug!(
        "Proxy handshake failed as expected: {:?}",
        result.err().unwrap().to_string()
    );
}

#[tokio::test]
async fn test_handler_request_channel() {
    let (ctrl_tx, ctrl_rx) = Handler::new_command_channel();
    let handler = Handler::new(ctrl_tx);

    let task = tokio::spawn(async move {
        if let Ok(cmd) = ctrl_rx.recv_async().await {
            match cmd {
                Command::RequestChannel {
                    channel_id,
                    response,
                } => {
                    assert_eq!(channel_id, 42);
                    // Create dummy channels to return
                    let (tx, _rx) = payload_with_channel_pair();
                    let (_tx2, rx) = payload_pair();

                    let channels = ServerChannels { tx, rx };
                    let _ = response.send_async(Ok(channels)).await;
                }
                _ => panic!("Unexpected command"),
            }
        }
    });

    let result = handler.request_channel(42).await;
    assert!(result.is_ok());
    task.await.unwrap();
}

#[tokio::test]
async fn test_handler_release_channel() {
    let (ctrl_tx, ctrl_rx) = Handler::new_command_channel();
    let handler = Handler::new(ctrl_tx);

    let task = tokio::spawn(async move {
        if let Ok(cmd) = ctrl_rx.recv_async().await {
            match cmd {
                Command::ReleaseChannel { channel_id } => {
                    assert_eq!(channel_id, 99);
                }
                _ => panic!("Unexpected command"),
            }
        }
    });

    let result = handler.release_channel(99).await;
    assert!(result.is_ok());
    task.await.unwrap();
}

#[tokio::test]
async fn test_connect() -> Result<()> {
    log::setup_logging("debug", log::LogType::Test);

    log::debug!("Creating proxy");
    let remote_server = dummy_remote_server().await;
    let stop = Trigger::new();
    let proxy = Proxy::new(
        &remote_server.addr,
        dummy_ticket(),
        dummy_crypt_info(),
        Duration::from_millis(100),
        stop.clone(),
    );

    proxy.run().await.context("Failed to run proxy")?;
    // If result is ok, the connection is done, data has been sent and received

    stop.trigger();
    Ok(())
}

#[tokio::test]
async fn test_recv_data() -> Result<()> {
    log::setup_logging("debug", log::LogType::Test);

    log::debug!("Creating proxy");
    let remote_server = dummy_remote_server().await;
    let proxy = Proxy::new(
        &remote_server.addr,
        dummy_ticket(),
        dummy_crypt_info(),
        Duration::from_millis(100),
        remote_server.stop.clone(),
    );

    let handler = proxy.run().await.context("Failed to run proxy")?;
    // Create a client
    let ServerChannels { tx: _tx, rx } = handler
        .request_channel(1)
        .await
        .context("Failed to request channel")?;

    // Send data to channel
    remote_server
        .tx
        .send_async(PayloadWithChannel::new(1, b"hello"))
        .await?;

    let data = rx
        .recv_async()
        .await
        .context("Failed to receive data from channel")?;

    log::debug!("Received data: {:?}", data);
    assert_eq!(data.as_ref(), b"hello");

    handler.release_channel(1).await?;

    remote_server.stop.trigger();
    Ok(())
}

#[tokio::test]
async fn test_send_data() -> Result<()> {
    log::setup_logging("debug", log::LogType::Test);

    log::debug!("Creating proxy");
    let remote_server = dummy_remote_server().await;
    let proxy = Proxy::new(
        &remote_server.addr,
        dummy_ticket(),
        dummy_crypt_info(),
        Duration::from_millis(100),
        remote_server.stop.clone(),
    );

    let handler = proxy.run().await.context("Failed to run proxy")?;
    // Create a client
    let ServerChannels { tx, rx: _rx } = handler
        .request_channel(1)
        .await
        .context("Failed to request channel")?;

    log::debug!("Sending data to channel");
    // Send data to channel
    tx.send_async(PayloadWithChannel::new(1, b"hello"))
        .await
        .context("Failed to send data to channel")?;

    log::debug!("Waiting for data from remote server");
    let data = remote_server
        .rx
        .recv_async()
        .await
        .context("Failed to receive data from remote server")?;

    log::debug!("Received data: {:?}", data);
    assert_eq!(data.payload.as_ref(), b"hello");
    assert_eq!(data.channel_id, 1);

    handler.release_channel(1).await?;

    remote_server.stop.trigger();
    Ok(())
}
