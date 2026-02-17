use anyhow::{Ok, Result};
use std::time::Duration;
use tokio::net::TcpListener;

use shared::log;

use crate::{consts::MAX_STARTUP_TIME_MS, registry, types::TunnelConnectInfo};

pub mod client;
pub mod protocol;
pub mod proxy;
pub mod server;
pub mod tunnel;

pub async fn tunnel_runner(info: TunnelConnectInfo, listener: TcpListener) -> Result<()> {
    log::debug!(
        "Starting tunnel runner with startup_time_ms: {}, max allowed: {}",
        info.startup_time_ms,
        MAX_STARTUP_TIME_MS
    );
    let (_id, trigger, active_connections) = registry::register_tunnel(Some(
        Duration::from_millis(info.startup_time_ms.min(MAX_STARTUP_TIME_MS)),
    ));
    Ok(())
}
