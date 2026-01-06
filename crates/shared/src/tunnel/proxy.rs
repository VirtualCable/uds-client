// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
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
use crate::system::trigger::Trigger;
use crate::tunnel::consts;
use anyhow::Result;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf, split},
    net::TcpStream,
};
use tokio_rustls::client::TlsStream;

use crate::log;

pub async fn start_proxy(
    mut tls_reader: ReadHalf<TlsStream<TcpStream>>,
    mut tls_writer: WriteHalf<TlsStream<TcpStream>>,
    client_stream: TcpStream,
    trigger: Trigger,
) -> Result<()> {
    let (mut client_reader, mut client_writer) = split(client_stream);

    // Task 1: client -> TLS
    let writer_task = tokio::spawn({
        let trigger = trigger.clone();
        async move {
            let mut buf = [0u8; consts::BUFFER_SIZE];
            loop {
                tokio::select! {
                    res = client_reader.read(&mut buf) => {
                        match res {
                            Ok(0) => {
                                // client closed, shut down TLS write side (close_notify will be sent)
                                let _ = tls_writer.shutdown().await;
                                break;
                            }
                            Ok(n) => {
                                if let Err(e) = tls_writer.write_all(&buf[..n]).await {
                                    log::error!("TLS write error: {e}");
                                    let _ = tls_writer.shutdown().await;
                                    break;
                                }
                            }
                            Err(e) => {
                                log::error!("Client read error: {e}");
                                let _ = tls_writer.shutdown().await;  // Ensure close_notify is sent
                                break;
                            }
                        }
                    }
                    _ = trigger.async_wait() => {
                        // Trigger fired, ensure send close_notify
                        let _ = tls_writer.shutdown().await;
                        break;
                    }
                }
            }
        }
    });

    // Task 2: TLS -> client
    let reader_task = tokio::spawn({
        let trigger = trigger.clone();
        async move {
            let mut buf = [0u8; consts::BUFFER_SIZE];
            loop {
                tokio::select! {
                    res = tls_reader.read(&mut buf) => {
                        match res {
                            Ok(0) => {
                                // tls closed, close client
                                let _ = client_writer.shutdown().await;
                                break;
                            }
                            Ok(n) => {
                                if let Err(e) = client_writer.write_all(&buf[..n]).await {
                                    log::debug!("Client write error: {e}");
                                    let _ = client_writer.shutdown().await;
                                    break;
                                }
                            }
                            Err(e) => {
                                log::debug!("TLS read error: {e}");
                                let _ = client_writer.shutdown().await;
                                break;
                            }
                        }
                    }
                    _ = trigger.async_wait() => {
                        // Trigger fired; close the client
                        let _ = client_writer.shutdown().await;
                        break;
                    }
                }
            }
        }
    });

    let _ = tokio::try_join!(writer_task, reader_task);
    Ok(())
}
