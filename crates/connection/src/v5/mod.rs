use anyhow::{Ok, Result};
use std::time::Duration;
use tokio::net::TcpListener;

use shared::log;

use crate::{consts::MAX_STARTUP_TIME_MS, registry, types::TunnelConnectInfo};

pub mod client;
pub mod protocol;
pub mod proxy;
pub mod server;

pub async fn tunnel_runner(info: TunnelConnectInfo, listener: TcpListener) -> Result<()> {
    log::debug!(
        "Starting tunnel runner with startup_time_ms: {}, max allowed: {}",
        info.startup_time_ms,
        MAX_STARTUP_TIME_MS
    );
    let (_id, trigger, active_connections) = registry::register_tunnel(Some(
        Duration::from_millis(info.startup_time_ms.min(MAX_STARTUP_TIME_MS)),
    ));
    let crypt_info = info.crypt.ok_or(anyhow::format_err!(
        "TunnelConnectInfo must include crypt material"
    ))?;

    loop {
        // Accept incoming connection until triggered to stop.
        tokio::select! {
            res = listener.accept() => {
                let (client_stream, client_addr) = res?;
                // Disable nagle's algorithm also on client side
                client_stream.set_nodelay(true).ok();

                log::debug!("Accepted connection from {}", client_addr);

                // Launch the proxy, register a client and launc it.
                // We will wait for client to end for cleanup
                // Currently, as only one channel is being used
                // this will be enough
                let proxy = proxy::Proxy::new(
                    &format!("{}:{}", info.addr, info.port),
                    info.ticket,
                    crypt_info,
                    std::time::Duration::from_millis(info.startup_time_ms.min(MAX_STARTUP_TIME_MS)),
                    trigger.clone(),
                ).run().await?;

                let (reader, writer) = client_stream.into_split();

                let channels = proxy.request_channel(1).await?;

                let server = server::TunnelServer::new(
                    reader,
                    writer,
                    1,
                    channels.tx.clone(),
                    channels.rx.clone(),
                    trigger.clone(),
                    proxy
                );

                log::debug!("Tunnel connection established, starting proxying");
                // Start proxying in a new task
                tokio::spawn({
                    let trigger = trigger.clone();
                    let active_connections = active_connections.clone();
                    async move {
                        active_connections.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if let Err(e) = server.run().await {
                            log::error!("Tunnel server error: {:?}", e);
                        }
                        active_connections.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
                        log::debug!("Tunnel connection closed, active connections: {}", active_connections.load(std::sync::atomic::Ordering::Relaxed));
                        trigger.trigger();  // Ensure our proxy is stopped
                    }
                });
            }
            _ = trigger.wait_async() => {
                log::info!("Tunnel runner triggered to stop accepting new connections.");
                break;
            }
        }
    }

    log::debug!("Tunnel runner exiting");
    // Ensure our trigger is set
    trigger.trigger();

    Ok(())
}

#[cfg(test)]
mod tests;
