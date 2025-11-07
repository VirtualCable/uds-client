#![allow(dead_code)]  // TODO: remove soon :)
use crate::system::trigger::Trigger;
use crate::tunnel::consts;
use anyhow::Result;
use tokio::io::{WriteHalf, ReadHalf, split};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
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
                        // Trigger fired, cerramos cliente
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
