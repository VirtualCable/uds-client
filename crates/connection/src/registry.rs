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
use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex, atomic::AtomicU32},
    time::{Duration, Instant},
};

use shared::{log, system::trigger::Trigger};

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

pub(super) fn register_tunnel(
    minimum_lifetime: Option<Duration>,
) -> (u32, Trigger, Arc<AtomicU32>) {
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
        log::debug!("Registered tunnel {}, minimum lifetime {} seconds", id, minimum_lifetime.as_secs());
        async move {
            trigger.wait_timeout_async(minimum_lifetime).await.ok();
            log::debug!("Minimum lifetime elapsed for tunnel {}, checking active connections", id);
            loop {
                // If already triggered, async_wait_timeout will return immediately
                if trigger.wait_timeout_async(Duration::from_secs(1)).await.is_ok()
                    || active_connections.load(std::sync::atomic::Ordering::Relaxed) == 0
                {
                    if !trigger.is_triggered() {
                        log::info!(
                            "Minimum lifetime elapsed and no active connections, stopping tunnel {}",
                            id
                        );
                    }
                    unregister_tunnel(id); // Will also set the trigger
                    break;
                }
                // Soft poll every second to check active connections
                trigger.wait_timeout_async(Duration::from_secs(1)).await.ok();
            }
        }
    });
    (id, trigger, active_connections)
}

pub(super) fn unregister_tunnel(tunnel_id: u32) {
    // Ensure stop trigger is activated
    if let Some(info) = TUNNEL_INFOS.lock().unwrap().get(&tunnel_id) {
        info.stop.trigger();
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
        info.stop.trigger();
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
