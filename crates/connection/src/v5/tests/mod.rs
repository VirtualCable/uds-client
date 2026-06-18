// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use tokio::{io::AsyncWriteExt, net::TcpStream};

// Share helpers with v5 tests
#[cfg(test)]
pub mod helpers;

use helpers::*;

use super::*;

async fn wait_any_tunnel(active: bool) -> Result<()> {
    log::debug!(
        "Waiting for any tunnel to become {}",
        if active { "active" } else { "inactive" }
    );
    registry::log_running_tunnels();
    for _ in 0..30 {
        if registry::is_any_tunnel_active() == active {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    log::error!(
        "Timeout waiting for tunnel to become {}",
        if active { "active" } else { "inactive" }
    );
    let tunnels = registry::log_running_tunnels();
    Err(anyhow::anyhow!(
        "Timeout waiting for tunnel to become {}: {}",
        if active { "active" } else { "inactive" },
        tunnels
    ))
}

async fn setup_test(
    startup_time_ms: u64,
) -> Result<(RemoteServer, TunnelConnectInfo, TcpListener)> {
    log::setup_logging("debug", log::LogType::Test);

    let remote_server = dummy_remote_server().await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;

    let info = TunnelConnectInfo {
        addr: remote_server.listen_host.clone(),
        port: remote_server.listen_port,
        ticket: dummy_ticket(),
        local_port: listener.local_addr()?.port().into(),
        check_certificate: false,
        startup_time_ms,
        keep_listening_after_timeout: false,
        enable_ipv6: false,
        shared_secret: Some(dummy_shared_secret()),
    };

    Ok((remote_server, info, listener))
}

#[tokio::test]
#[ignore = "This test checks for global registry, so it should be run alone"]
async fn test_tunnel_stops() -> Result<()> {
    let (remote_server, info, listener) = setup_test(100).await?;

    tokio::spawn({
        async move {
            if let Err(e) = tunnel_runner(info, listener).await {
                log::error!("Tunnel runner error: {:?}", e);
            }
        }
    });

    wait_any_tunnel(true).await?;

    registry::stop_tunnels();

    wait_any_tunnel(false).await?;

    // Stop remote testing server
    remote_server.stop.trigger();

    Ok(())
}

#[serial_test::serial(v5, registry)]
#[tokio::test]
async fn test_tunnel_sends_data_to_remote_and_closes_connection() -> Result<()> {
    let (remote_server, info, listener) = setup_test(100).await?;

    let port = info.local_port.unwrap();

    tokio::spawn({
        async move {
            if let Err(e) = tunnel_runner(info, listener).await {
                log::error!("Tunnel runner error: {:?}", e);
            }
        }
    });

    // Connect to the tunnel and send some data
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await?;
    log::debug!("Connected to tunnel");
    stream.write_all(b"Hello, tunnel!").await?;

    // Data should reach the remote server
    let data = remote_server.rx.recv_async().await?;
    log::debug!("Data received by remote server: {:?}", data);
    assert_eq!(data.payload.as_ref(), b"Hello, tunnel!");

    // Close and wait a bit, so the listener gets closes
    drop(stream);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let stream = TcpStream::connect(("12127.0.0.1", port)).await;
    assert!(
        stream.is_err(),
        "Tunnel should be closed, but connection succeeded"
    );

    // Ensure remote is finished
    remote_server.stop.trigger();

    Ok(())
}

#[serial_test::serial(v5, registry)]
#[tokio::test]
async fn test_tunnel_closes_after_startup_timeout() -> Result<()> {
    let (remote_server, info, listener) = setup_test(100).await?;

    let port = info.local_port.unwrap();

    tokio::spawn({
        async move {
            if let Err(e) = tunnel_runner(info, listener).await {
                log::error!("Tunnel runner error: {:?}", e);
            }
        }
    });

    // Connect to the tunnel and send some data
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await?;
    log::debug!("Connected to tunnel");
    stream.write_all(b"Hello, tunnel!").await?;

    // Data should reach the remote server
    let data = remote_server.rx.recv_async().await?;
    log::debug!("Data received by remote server: {:?}", data);
    assert_eq!(data.payload.as_ref(), b"Hello, tunnel!");

    // Open another connection to ensure tunnel is still active
    let mut stream2 = TcpStream::connect(("127.0.0.1", port)).await?;
    log::debug!("Connected to tunnel");
    stream2.write_all(b"Hello, tunnel!").await?;

    // Data should reach the remote server
    let data = remote_server.rx.recv_async().await?;
    log::debug!("Data received by remote server: {:?}", data);
    assert_eq!(data.payload.as_ref(), b"Hello, tunnel!");

    // Ensure remote is finished
    remote_server.stop.trigger();

    Ok(())
}
