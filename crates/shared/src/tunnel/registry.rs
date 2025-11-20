use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex, atomic::AtomicU32},
    time::{Duration, Instant},
};

use crate::{log, system::trigger::Trigger};

struct TunnelInfo {
    pub started_at: Instant,        // When the tunnel was started
    pub minimum_lifetime: Duration, // Minimum lifetime before it can be stopped when no connections has been made
    pub stop: Trigger,              // Trigger to stop the tunnel
    pub active_connections: Arc<AtomicU32>,
}

impl TunnelInfo {
    pub fn can_be_stopped(&self) -> bool {
        Instant::now().duration_since(self.started_at) >= self.minimum_lifetime
    }
}

static TUNNEL_HANDLE_COUNTER: AtomicU32 = AtomicU32::new(1);
static TUNNEL_INFOS: LazyLock<Mutex<HashMap<u32, TunnelInfo>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) fn register_tunnel(minimum_lifetime: Option<Duration>) -> (u32, Trigger, Arc<AtomicU32>) {
    let id = TUNNEL_HANDLE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let trigger = Trigger::new();
    let active_connections = Arc::new(AtomicU32::new(0));
    let minimum_lifetime = minimum_lifetime.unwrap_or(Duration::from_secs(30));
    TUNNEL_INFOS.lock().unwrap().insert(
        id,
        TunnelInfo {
            started_at: Instant::now(),
            minimum_lifetime,
            stop: trigger.clone(),
            active_connections: active_connections.clone(),
        },
    );

    // Spawns a task that after minimum_lifetime, if no connections, unregisters the tunnel (thats triggers stop in time)
    tokio::spawn({
        let trigger = trigger.clone();
        let active_connections = active_connections.clone();
        async move {
            trigger.async_wait_timeout(minimum_lifetime).await;
            loop {
                // If already triggered, async_wait_timeout will return immediately
                if trigger.async_wait_timeout(Duration::from_secs(1)).await
                    || active_connections.load(std::sync::atomic::Ordering::Relaxed) == 0
                {
                    if !trigger.is_set() {
                        log::info!(
                            "Minimum lifetime elapsed and no active connections, stopping tunnel {}",
                            id
                        );
                    }
                    unregister_tunnel(id); // Will also set the trigger
                    break;
                }
                // Soft poll every second to check active connections
                trigger.async_wait_timeout(Duration::from_secs(1)).await;
            }
        }
    });
    (id, trigger, active_connections)
}

pub(super) fn unregister_tunnel(tunnel_id: u32) {
    // Ensure stop trigger is activated
    if let Some(info) = TUNNEL_INFOS.lock().unwrap().get(&tunnel_id) {
        info.stop.set();
    }
    TUNNEL_INFOS.lock().unwrap().remove(&tunnel_id);
}

pub fn is_any_tunnel_active() -> bool {
    let infos = TUNNEL_INFOS.lock().unwrap();
    !infos.is_empty()
}

#[allow(dead_code)]
pub fn stop_tunnels() {
    let infos = TUNNEL_INFOS.lock().unwrap();
    for (_id, info) in infos.iter() {
        info.stop.set();
    }
}

#[allow(dead_code)]
pub fn log_running_tunnels() {
    let infos = TUNNEL_INFOS.lock().unwrap();
    for (id, info) in infos.iter() {
        log::debug!(
            "Tunnel {}: started at {:?}, can_be_stopped {}, active connections {}",
            id,
            info.started_at,
            info.can_be_stopped(),
            info.active_connections
                .load(std::sync::atomic::Ordering::Relaxed)
        );
    }
}
