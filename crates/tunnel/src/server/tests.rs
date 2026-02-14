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
use crate::{protocol::Payload, proxy::Command};

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use super::*;

use tokio::io::{DuplexStream, ReadHalf, WriteHalf};

struct TestContext {
    server: TunnelServer<ReadHalf<DuplexStream>, WriteHalf<DuplexStream>>,
    local: DuplexStream,
    ctrl_tx: flume::Sender<Command>, // To keep the channel alive on our tests
    ctrl_rx: flume::Receiver<Command>,
    payload_tx: flume::Sender<Payload>,
    payload_rx: flume::Receiver<PayloadWithChannel>,
    stop: Trigger,
}

fn create_server(channel_id: u16) -> TestContext {
    shared::log::setup_logging("debug", shared::log::LogType::Test);

    let (client, local) = tokio::io::duplex(1024);
    let (client_tx, payload_rx) = flume::bounded(10);
    let (payload_tx, client_rx) = flume::bounded(10);
    let (ctrl_tx, ctrl_rx) = flume::bounded(1);

    let (client_reader, client_writer) = tokio::io::split(client);

    let stop = Trigger::new();

    // Crate a tunnel server with async-everything to ease testing
    TestContext {
        server: TunnelServer {
            reader: client_reader,
            writer: client_writer,
            channel_id,
            tx: client_tx,
            rx: client_rx,
            stop: stop.clone(),
            proxy_ctrl: Handler::new(ctrl_tx.clone()),
        },
        local,
        ctrl_tx,
        ctrl_rx,
        payload_tx,
        payload_rx,
        stop,
    }
}

#[tokio::test]
async fn check_stop() {
    let TestContext {
        server,
        local: _local,
        // keep Channels alive so the don't get closed when going out of scope
        ctrl_tx: _ctrl_tx,
        ctrl_rx,
        payload_tx: _payload_tx,
        payload_rx: _payload_rx,
        stop,
    } = create_server(1);

    let stopped = Trigger::new(); // used to signal expected run completion
    tokio::spawn({
        let stopped = stopped.clone();
        async move {
            // Wait a bit to let the server start
            if let Err(e) = server.run().await {
                log::error!("Server error: {:?}", e);
            } else {
                log::info!("Server run completed successfully");
                stopped.trigger(); // Signal that the server has stopped
            }
        }
    });

    // Send stop command
    stop.trigger();

    // no message on ctrl_rx, ensure
    assert!(
        ctrl_rx.try_recv().is_err(),
        "Expected no commands to be sent to proxy after stop"
    );

    stopped
        .wait_timeout_async(std::time::Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn check_remote_connection_closed() {
    let TestContext {
        server,
        local,
        // keep Channels alive so the don't get closed when going out of scope
        ctrl_tx: _ctrl_tx,
        ctrl_rx,
        payload_tx: _payload_tx,
        payload_rx: _payload_rx,
        stop: _stop,
    } = create_server(1);

    let stopped = Trigger::new(); // used to signal expected run completion
    tokio::spawn({
        let stopped = stopped.clone();
        async move {
            // Wait a bit to let the server start
            if let Err(e) = server.run().await {
                log::error!("Server error: {:?}", e);
            } else {
                log::info!("Server run completed successfully");
                stopped.trigger(); // Signal that the server has stopped
            }
        }
    });

    // Close the local end, simulating remote connection closure
    drop(local);

    // Should receive channel_release command on ctrl_rx
    match ctrl_rx.recv_async().await {
        Ok(Command::ReleaseChannel { channel_id }) => {
            assert_eq!(channel_id, 1, "Expected channel_id 1 to be released");
        }
        Ok(cmd) => {
            panic!("Expected ReleaseChannel command, got {:?}", cmd);
        }
        Err(e) => {
            panic!("Failed to receive command from proxy: {:?}", e);
        }
    }

    // No more message on ctrl_rx, ensure
    assert!(ctrl_rx.try_recv().is_err(),);

    stopped
        .wait_timeout_async(std::time::Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn inbound_chan_closed_works() {
    let TestContext {
        server,
        local: _local,
        // keep Channels alive so the don't get closed when going out of scope
        ctrl_tx: _ctrl_tx,
        ctrl_rx,
        payload_tx,
        payload_rx: _payload_rx,
        stop: _stop,
    } = create_server(1);

    let stopped = Trigger::new(); // used to signal expected run completion
    tokio::spawn({
        let stopped = stopped.clone();
        async move {
            // Wait a bit to let the server start
            if let Err(e) = server.run().await {
                log::info!("Server got error as expected: {:?}", e.to_string());
                stopped.trigger(); // Signal that the server has stopped
            } else {
                log::error!("Server run completed successfully");
            }
        }
    });

    drop(payload_tx); // Close the payload channel to simulate proxy connection loss

    // If not stopped in time, it's a failure, as it means the client did not detect the channel closure
    stopped
        .wait_timeout_async(std::time::Duration::from_secs(1))
        .await
        .unwrap();

    // No message on ctrl_rx, ensure
    assert!(
        ctrl_rx.try_recv().is_err(),
        "Expected no commands to be sent to proxy after channel closure"
    );
}

#[tokio::test]
async fn outbound_chan_closed_works() {
    let TestContext {
        server,
        mut local, // We need to keep the cannels alive, event if not used
        ctrl_tx: _ctrl_tx,
        ctrl_rx,
        payload_tx: _payload_tx,
        payload_rx,
        stop: _stop,
        ..
    } = create_server(1);

    let stopped = Trigger::new(); // used to signal expected run completion
    tokio::spawn({
        let stopped = stopped.clone();
        async move {
            // Wait a bit to let the server start
            if let Err(e) = server.run().await {
                log::info!("Server got error as expected: {:?}", e.to_string());
                stopped.trigger(); // Signal that the server has stopped
            } else {
                log::error!("Server run completed successfully");
            }
        }
    });

    drop(payload_rx);
    // Write something so the server tries to send data
    if let Err(e) = local.write_all(b"test").await {
        log::info!(
            "Failed to write to local stream as expected: {:?}",
            e.to_string()
        );
    } else {
        log::error!("Write to local stream succeeded unexpectedly");
    }

    // If not stopped in time, it's a failure, as it means the client did not detect the channel closure
    stopped
        .wait_timeout_async(std::time::Duration::from_secs(1))
        .await
        .unwrap();

    // No message on ctrl_rx, ensure
    let result = ctrl_rx.try_recv();
    assert!(
        result.is_err(),
        "Expected no commands to be sent to proxy after channel closure: got {:?}",
        result
    );
}

#[tokio::test]
async fn sends_data() {
    let TestContext {
        server,
        mut local,
        // We need to keep the channels alive, event if not used
        ctrl_tx: _ctrl_tx,
        ctrl_rx: _ctrl_rx,
        payload_tx,
        payload_rx: _payload_rx,
        stop,
        ..
    } = create_server(1);
    tokio::spawn({
        let stop = stop.clone();
        async move {
            // Run the client, it should stop when we receive connection closed from server
            if let Err(e) = server.run().await {
                log::error!("Server run failed: {:?}", e);
            } else {
                log::info!("Server run completed successfully");
            }
            log::info!("Server run completed");
            stop.trigger(); // Signal that the server has stopped
        }
    });

    // Send something using payload_tx
    payload_tx
        .send_async(Payload::new(b"test"))
        .await
        .unwrap();

    // Read from local
    let mut buf = [0u8; 4];
    local.read_exact(&mut buf).await.unwrap();

    assert_eq!(&buf, b"test");

    stop.trigger(); // Stop the client
}

#[tokio::test]
async fn receives_data() {
    let TestContext {
        server,
        mut local,
        // We need to keep the channels alive, event if not used
        ctrl_tx: _ctrl_tx,
        ctrl_rx: _ctrl_rx,
        payload_tx: _payload_tx,
        payload_rx,
        stop,
        ..
    } = create_server(1);

    tokio::spawn({
        let stop = stop.clone();
        async move {
            // Run the server, it should stop when we receive connection closed from client
            if let Err(e) = server.run().await {
                log::error!("Server run failed: {:?}", e);
            } else {
                log::info!("Server run completed successfully");
            }
            log::info!("Server run completed");
            stop.trigger(); // Signal that the server has stopped
        }
    });

    // Send something using local, encrypting it first
    local.write_all(b"test").await.unwrap();

    // Read from payload_rx
    let payload = payload_rx.recv_async().await.unwrap();
    assert_eq!(payload.channel_id, 1);
    assert_eq!(payload.payload.as_ref(), b"test");

    stop.trigger(); // Stop the client
}
