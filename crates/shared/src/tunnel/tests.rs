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
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{
    connection::{connect_and_upgrade, send_open_cmd, send_test_cmd},
    test_utils::{connect, create_runner, create_ticket},
};
use crate::log;

// use super::{consts, proxy};

#[tokio::test]
async fn test_connect_and_upgrade() {
    let (reader, mut writer, server_handle, trigger) = connect(None, 44913)
        .await
        .expect("Failed to connect to test server");

    // If we reach here, the connection and upgrade were successful
    log::debug!("Connected and upgraded to TLS successfully");
    writer.shutdown().await.ok();
    drop(writer);
    drop(reader);
    trigger.set();
    server_handle.await.unwrap();
}

#[tokio::test]
async fn test_test_cmd() {
    let (mut reader, mut writer, server_handle, trigger) = connect(None, 44914)
        .await
        .expect("Failed to connect to test server");

    // Send CMD_TEST
    send_test_cmd(&mut reader, &mut writer).await.unwrap(); //will panic on error

    log::debug!("Test command tests completed successfully");
    writer.shutdown().await.ok();
    drop(writer);
    drop(reader);
    trigger.set();
    server_handle.await.unwrap();
}

#[tokio::test]
async fn test_open_cmd() {
    let (mut reader, mut writer, server_handle, trigger) = connect(None, 44915)
        .await
        .expect("Failed to connect to test server");

    // Send CMD_OPEN with a ticket
    //consts::TICKET_LENGTH
    let rnd_ticket = create_ticket();
    send_open_cmd(&mut reader, &mut writer, &rnd_ticket)
        .await
        .unwrap(); //will panic on error

    log::debug!("Text command tests completed successfully");
    writer.shutdown().await.ok();
    drop(writer);
    drop(reader);
    trigger.set();
    server_handle.await.unwrap();
}

#[tokio::test]
async fn test_connect_and_upgrade_invalid_server() {
    log::setup_logging("debug", log::LogType::Tests);
    crate::tls::init_tls(None);
    log::debug!("Starting test_connect_and_upgrade_invalid_server");
    let result = connect_and_upgrade("invalid.server.name", 44916, false).await;
    assert!(result.is_err(), "Connection to invalid server should fail");
    log::debug!("test_connect_and_upgrade_invalid_server completed successfully");
}

// Even we use diferrent ports, STOP_TRIGGER is shared, so tests must be serialized
#[tokio::test]
async fn test_tunnel_runner_starts_and_stop() {
    let (server_handle, runner_handle, listen_port, trigger) = create_runner(44916)
        .await
        .expect("Failed to create test server and runner");

    log::debug!("Test server and runner started on port {}", listen_port);

    // Stop the server and runner
    trigger.set();

    runner_handle.await.unwrap();
    server_handle.await.unwrap();

    log::debug!("test_tunnel_runner_starts_and_stop completed successfully");
}

#[tokio::test]
async fn test_tunnel_runner_some_data() {
    let (server_handle, runner_handle, listen_port, trigger) = create_runner(44917)
        .await
        .expect("Failed to create test server and runner");

    log::debug!("Test server and runner started on port {}", listen_port);

    // Connect to localhost, port listen_port and send some data. Must echo back.
    let mut conn = tokio::net::TcpStream::connect(("localhost", listen_port))
        .await
        .expect("Failed to connect to tunnel");
    log::debug!("Connected to tunnel on port {}", listen_port);
    // A buffer with 256 bytes, each byte set to its index value (0, 1, 2, ..., 255)
    let mut test_data = vec![0u8; 256];
    for (i, item) in test_data.iter_mut().enumerate() {
        *item = i as u8;
    }
    // Send a number of times the pattern varying the values by 1 each time
    for _i in 0..32 {
        conn.writable().await.expect("Connection not writable");
        conn.write_all(&test_data)
            .await
            .expect("Failed to write test data");
        log::debug!("Sent test data to tunnel");
        let mut buf = vec![0u8; test_data.len()];
        conn.readable().await.expect("Connection not readable");
        let n = conn
            .read(&mut buf)
            .await
            .expect("Failed to read echoed data");
        assert_eq!(
            &buf[..n],
            &test_data[..n],
            "Echoed data does not match sent data"
        );
        // Increment each byte in test_data by 1 for next iteration
        for item in test_data.iter_mut() {
            *item = item.wrapping_add(1);
        }
    }
    conn.shutdown()
        .await
        .expect("Failed to shutdown connection");
    log::debug!("Sent test and open commands successfully");

    trigger.set();

    log::debug!("Stopping runner");
    runner_handle.await.unwrap();
    log::debug!("Stopping server");
    server_handle.await.unwrap();

    log::debug!("test_tunnel_runner_some_data completed successfully");
}
