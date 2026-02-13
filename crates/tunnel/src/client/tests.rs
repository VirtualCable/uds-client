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
use crate::{
    crypt::{Crypt, types::SharedSecret},
    proxy::Command,
};

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use super::*;

use tokio::io::{DuplexStream, ReadHalf, WriteHalf};

struct TestContext {
    client: TunnelClient<ReadHalf<DuplexStream>, WriteHalf<DuplexStream>>,
    local: DuplexStream,
    ctrl_tx: flume::Sender<Command>, // To keep the channel alive on our tests
    ctrl_rx: flume::Receiver<Command>,
    payload_tx: flume::Sender<PayloadWithChannel>,
    payload_rx: flume::Receiver<PayloadWithChannel>,
    crypt_inbound: Crypt,
    crypt_outbound: Crypt,
    stop: Trigger,
}

fn create_client() -> TestContext {
    shared::log::setup_logging("debug", shared::log::LogType::Test);

    let (client, local) = tokio::io::duplex(1024);
    let (client_tx, payload_rx) = flume::bounded(10);
    let (payload_tx, client_rx) = flume::bounded(10);
    let (ctrl_tx, ctrl_rx) = flume::bounded(1);

    let (client_reader, client_writer) = tokio::io::split(client);
    let secret_in = SharedSecret::new([1; 32]);
    let secret_out = SharedSecret::new([2; 32]);

    let stop = Trigger::new();

    let crypt_inbound = Crypt::new(&secret_in, 0);
    let crypt_outbound = Crypt::new(&secret_out, 16);

    // Crate a tunnel client with async-everything to ease testing
    TestContext {
        client: TunnelClient {
            reader: client_reader,
            writer: client_writer,
            tx: client_tx,
            rx: client_rx,
            crypt_inbound: crypt_inbound.clone(),
            crypt_outbound: crypt_outbound.clone(),
            stop: stop.clone(),
            proxy_ctrl: Handler::new(ctrl_tx.clone()),
        },
        local,
        ctrl_tx,
        ctrl_rx,
        crypt_inbound,
        crypt_outbound,
        payload_tx,
        payload_rx,
        stop,
    }
}

#[tokio::test]
async fn check_stop() {
    let TestContext {
        client,
        stop,
        // We need to keep the cannels alive, event if not used
        ctrl_tx: _ctrl_tx,
        ctrl_rx,
        payload_tx: _payload_tx,
        payload_rx: _payload_rx,
        ..
    } = create_client();

    let stopped = Trigger::new(); // used to signal test completion
    tokio::spawn({
        let stopped = stopped.clone();
        async move {
            // Run the client, it should stop when we send the stop signal
            client.run(None).await.unwrap();
            stopped.trigger(); // Signal that the client has stopped
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
        client,
        local,
        // We need to keep the cannels alive, event if not used
        ctrl_tx: _ctrl_tx,
        ctrl_rx,
        payload_tx: _payload_tx,
        payload_rx: _payload_rx,
        ..
    } = create_client();
    let stopped = Trigger::new(); // used to signal test completion
    tokio::spawn({
        let stopped = stopped.clone();
        async move {
            // Run the client, it should stop when we receive connection closed from server
            if let Err(e) = client.run(None).await {
                log::error!("Client run failed: {:?}", e);
            } else {
                log::info!("Client run completed successfully");
            }
            log::info!("Client run completed");
            stopped.trigger(); // Signal that the client has stopped
        }
    });

    // Close the local end, which simulates the server closing the connection
    drop(local);

    // We should receive a ConnectionClosed command in the proxy
    match ctrl_rx.recv_async().await.unwrap() {
        Command::ConnectionClosed => {
            // Expected, do nothing
        }
        other => panic!("Expected ConnectionClosed command, got {:?}", other),
    }

    stopped
        .wait_timeout_async(std::time::Duration::from_secs(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn inbound_chan_closed_works_finely() {
    let TestContext {
        client,
        local: _local,
        ctrl_tx: _ctrl_tx,
        ctrl_rx,
        payload_tx,
        payload_rx: _payload_rx,
        ..
    } = create_client();
    let stopped = Trigger::new(); // used to signal test completion
    tokio::spawn({
        let stopped = stopped.clone();
        async move {
            // Run the client, it should stop when we receive connection closed from server
            if let Err(e) = client.run(None).await {
                // Must return err, because chanel is closed
                log::info!("Client run failed as expected: {}", e);
                stopped.trigger(); // Signal that the client has stopped
            } else {
                log::error!(
                    "Client run completed successfully, expected failure due to channel closure"
                );
            }
            log::info!("Client run completed");
        }
    });

    drop(payload_tx);
    // Send something using locak, to ensure data is got

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
async fn outbound_chan_closed_works_finely() {
    let TestContext {
        client,
        mut local, // We need to keep the cannels alive, event if not used
        ctrl_tx: _ctrl_tx,
        ctrl_rx,
        payload_tx: _payload_tx,
        payload_rx,
        mut crypt_outbound,
        stop,
        ..
    } = create_client();
    let stopped = Trigger::new(); // used to signal test completion
    tokio::spawn({
        let stopped = stopped.clone();
        async move {
            // Run the client, it should stop when we receive connection closed from server
            if client.run(None).await.is_err() {
                // Must return err, because chanel is closed
                log::info!("Client run failed as expected:");
                stopped.trigger(); // Signal that the client has stopped
            } else {
                log::error!(
                    "Client run completed successfully, expected failure due to channel closure"
                );
            }
            log::info!("Client run completed");
        }
    });

    drop(payload_rx);

    // Sends a valid packet, but as the channel is closed, it should cause the client to stop with an error
    crypt_outbound
        .write(&stop, &mut local, 1, b"test")
        .await
        .unwrap();

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
async fn sends_data() {
    let TestContext {
        client,
        mut local,
        // We need to keep the channels alive, event if not used
        ctrl_tx: _ctrl_tx,
        ctrl_rx: _ctrl_rx,
        payload_tx,
        payload_rx: _payload_rx,
        mut crypt_outbound,
        stop,
        ..
    } = create_client();
    tokio::spawn({
        let stop = stop.clone();
        async move {
            // Run the client, it should stop when we receive connection closed from server
            if let Err(e) = client.run(None).await {
                log::error!("Client run failed: {:?}", e);
            } else {
                log::info!("Client run completed successfully");
            }
            log::info!("Client run completed");
            stop.trigger(); // Signal that the client has stopped
        }
    });

    // Send something using payload_tx
    payload_tx
        .send_async(PayloadWithChannel::new(1, b"test"))
        .await
        .unwrap();

    // Read from local and decrypt
    let mut buffer = PacketBuffer::new();
    let (data, channel_id) = crypt_outbound
        .read(&stop, &mut local, &mut buffer)
        .await
        .unwrap();

    assert_eq!(channel_id, 1);
    assert_eq!(data, b"test");

    stop.trigger(); // Stop the client
}

#[tokio::test]
async fn receives_data() {
    let TestContext {
        client,
        mut local,
        // We need to keep the channels alive, event if not used
        ctrl_tx: _ctrl_tx,
        ctrl_rx: _ctrl_rx,
        payload_tx: _payload_tx,
        payload_rx,
        mut crypt_inbound,
        stop,
        ..
    } = create_client();

    tokio::spawn({
        let stop = stop.clone();
        async move {
            // Run the client, it should stop when we receive connection closed from server
            if let Err(e) = client.run(None).await {
                log::error!("Client run failed: {:?}", e);
            } else {
                log::info!("Client run completed successfully");
            }
            log::info!("Client run completed");
            stop.trigger(); // Signal that the client has stopped
        }
    });

    // Send something using local, encrypting it first
    crypt_inbound
        .write(&stop, &mut local, 1, b"test")
        .await
        .unwrap();

    // Read from payload_rx
    let payload = payload_rx.recv_async().await.unwrap();
    assert_eq!(payload.channel_id, 1);
    assert_eq!(payload.payload.as_ref(), b"test");

    stop.trigger(); // Stop the client
}
