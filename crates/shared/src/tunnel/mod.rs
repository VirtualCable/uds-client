use anyhow::Result;
use tokio::net::TcpListener;

mod connection;
mod consts;
mod proxy;

use crate::{log, system::trigger::Trigger};

pub struct TunnelConnectInfo {
    pub addr: String,
    pub port: u16,
    pub ticket: String,
    pub local_port: Option<u16>, // It None, a random port will be used
    pub check_certificate: bool, // whether to check server certificate
    pub listen_timeout_ms: u64,  // Timeout for listening
    pub keep_listening_after_timeout: bool, // whether to keep listening after timeout
    pub enable_ipv6: bool,       // whether to enable ipv6 (local and remote)
}

pub static STOP_TRIGGER: std::sync::LazyLock<Trigger> = std::sync::LazyLock::new(Trigger::new);

pub async fn tunnel_runner(info: TunnelConnectInfo, listener: TcpListener) -> Result<()> {
    let trigger = STOP_TRIGGER.clone();

    loop {
        // Accept incoming connection until triggered
        let accept_fut = listener.accept();
        tokio::select! {
            res = accept_fut => {
                let (client_stream, client_addr) = res?;
                // Disable nagle's algorithm also on client side
                client_stream.set_nodelay(true).ok();

                crate::log::info!("Accepted connection from {}", client_addr);
                // Open connection, no new test is needed here since we already tested in start_tunnel
                let (mut reader, mut writer) = connection::connect_and_upgrade(
                    &info.addr,
                    info.port,
                    info.check_certificate,
                ).await?;
                connection::send_open_cmd(&mut reader, &mut writer, &info.ticket).await?;
                log::debug!("Tunnel connection established, starting proxying");
                // Start proxying in a new task
                tokio::spawn({
                    let trigger = trigger.clone();
                    async move {
                        if let Err(e) = proxy::start_proxy(
                            reader,
                            writer,
                            client_stream,
                            trigger,
                        ).await {
                            crate::log::error!("Proxy error: {e}");
                        }
                    }
                });
            }
            _ = trigger.async_wait() => {
                crate::log::info!("Tunnel runner triggered to stop accepting new connections.");
                break;
            }
        }
    }

    Ok(())
}

pub async fn start_tunnel(info: TunnelConnectInfo) -> Result<u16> {
    // This works this way:
    // 0. Connect to remote server and upgrade to TLS, test connection and close initial connection. (for early failure detection)
    // 1. Listen to local port (info.local_port or random)
    // 2. On connection, connect to remote server and upgrade to TLS
    // 3. Open
    // 3. Start proxying data between local port and TLS connection

    log::debug!("Sending initial test connection to tunnel server");
    {
        let (mut reader, mut writer) =
            connection::connect_and_upgrade(&info.addr, info.port, info.check_certificate).await?;

        // Test to ensure connection is valid
        connection::send_test_cmd(&mut reader, &mut writer).await?;
    }
    log::debug!("Initial test connection successful");

    log::debug!("Creating local listener");
    // Open listener here to get the actual port, but move the listener into the tunnel runner
    let listener = connection::create_listener(info.local_port, info.enable_ipv6).await?;
    let actual_port = listener.local_addr()?.port();

    log::info!(
        "Tunnel listening on port {}, forwarding to {}:{}",
        actual_port,
        info.addr,
        info.port
    );
    tokio::spawn({
        async move {
            if let Err(e) = tunnel_runner(info, listener).await {
                crate::log::error!("Tunnel error: {e}");
            }
        }
    });

    Ok(actual_port)
}

#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod tests;
