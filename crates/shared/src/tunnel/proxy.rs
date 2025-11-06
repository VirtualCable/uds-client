use crate::system::trigger::Trigger;
use anyhow::Result;
use tokio::io::split;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::runtime::Builder;
use tokio_rustls::TlsStream;

async fn start_proxy(
    tls_stream: TlsStream<TcpStream>,
    client_stream: TcpStream,
    trigger: Trigger,
) -> Result<()> {
    let (mut tls_reader, mut tls_writer) = split(tls_stream);
    let (mut client_reader, mut client_writer) = split(client_stream);

    // Task 1: client -> TLS
    let writer_task = tokio::spawn({
        let trigger = trigger.clone();
        async move {
            let mut buf = [0u8; 16 * 1024];
            loop {
                tokio::select! {
                    res = client_reader.read(&mut buf) => {
                        match res {
                            Ok(0) => break, // client closed
                            Ok(n) => {
                                if let Err(e) = tls_writer.write_all(&buf[..n]).await {
                                    eprintln!("TLS write error: {e}");
                                    break;
                                }
                            }
                            Err(e) => {
                                eprintln!("Client read error: {e}");
                                break;
                            }
                        }
                    }
                    _ = trigger.async_wait() => {
                        // Trigger fired, exit loop
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
            let mut buf = [0u8; 16 * 1024];
            loop {
                tokio::select! {
                    res = tls_reader.read(&mut buf) => {
                        match res {
                            Ok(0) => break, // TLS closed
                            Ok(n) => {
                                if let Err(e) = client_writer.write_all(&buf[..n]).await {
                                    eprintln!("Client write error: {e}");
                                    break;
                                }
                            }
                            Err(e) => {
                                eprintln!("TLS read error: {e}");
                                break;
                            }
                        }
                    }
                    _ = trigger.async_wait() => {
                        break;
                    }
                }
            }
        }
    });

    let _ = tokio::try_join!(writer_task, reader_task);
    Ok(())
}

pub fn spawn_proxy_thread(
    tls_stream: TlsStream<TcpStream>,
    client_stream: TcpStream,
    trigger: Trigger,
) {
    std::thread::spawn(move || {
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async move {
            if let Err(e) = start_proxy(tls_stream, client_stream, trigger).await {
                eprintln!("Proxy error: {e}");
            }
        });
    });
}
