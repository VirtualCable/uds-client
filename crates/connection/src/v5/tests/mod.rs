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

use tokio::{net::TcpStream, io::AsyncWriteExt};

// Share helpers with v5 tests
#[cfg(test)]
pub mod helpers;

use helpers::*;

use super::*;

async fn wait_any_tunnel(active: bool) -> Result<()> {
    for _ in 0..10 {
        if registry::is_any_tunnel_active() == active {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err(anyhow::anyhow!("No tunnel became active"))
}

async fn setup_test() -> Result<(RemoteServer, TunnelConnectInfo, TcpListener)> {
    log::setup_logging("debug", log::LogType::Test);

    let remote_server = dummy_remote_server().await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;

    let info = TunnelConnectInfo {
        addr: remote_server.listen_host.clone(),
        port: remote_server.listen_port,
        ticket: dummy_ticket(),
        local_port: listener.local_addr()?.port().into(),
        check_certificate: false,
        startup_time_ms: 100,
        keep_listening_after_timeout: false,
        enable_ipv6: false,
        crypt: Some(dummy_crypt_info()),
    };

    Ok((remote_server, info, listener))
}

#[tokio::test]
async fn test_tunnel_stops() -> Result<()> {
    let (remote_server, info, listener) = setup_test().await?;

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

#[tokio::test]
async fn test_tunnel_sends_data_to_remote_connection() -> Result<()> {
    let (remote_server, info, listener) = setup_test().await?;

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

    wait_any_tunnel(true).await?;

    // Data should reach the remote server
    let data = remote_server.rx.recv_async().await?;
    log::debug!("Data received by remote server: {:?}", data);
    assert_eq!(data.payload.as_ref(), b"Hello, tunnel!");

    // Stop remote testing server
    remote_server.stop.trigger();

    wait_any_tunnel(false).await?;

    Ok(())
}
