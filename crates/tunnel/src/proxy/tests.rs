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

use tokio::{io::AsyncWriteExt, net::TcpListener};

use shared::log;

use crate::{crypt::types::SharedSecret, protocol::ticket::Ticket};

use super::*;

// Helper to create dummy ticket
fn dummy_ticket() -> Ticket {
    Ticket::new([0u8; 48])
}

// Helper to create dummy shared secret
fn dummy_shared_secret() -> SharedSecret {
    SharedSecret::new([0u8; 32])
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
        dummy_shared_secret(),
        Duration::from_millis(100),
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
        dummy_shared_secret(),
        Duration::from_millis(100),
    );

    // Should fail during handshake
    let result = proxy.run().await;
    assert!(result.is_err());
    log::debug!("Proxy handshake failed as expected: {:?}", result.err().unwrap().to_string());
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
                    let (tx, _rx) = crate::protocol::payload_with_channel_pair();
                    let (_tx2, rx) = crate::protocol::payload_pair();

                    let channels = crate::proxy::handler::ServerChannels { tx, rx };
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
