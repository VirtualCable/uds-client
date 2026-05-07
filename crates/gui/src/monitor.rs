// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
// Monitor geometry, queried from winit at startup via RdpAppProxy::resumed.
use std::sync::OnceLock;

use winit::{event_loop::ActiveEventLoop, monitor::MonitorHandle};

/// Monitor geometry queried from winit at startup.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub size: (u32, u32),
    pub position: (i32, i32),
    pub scale: f64,
}

/// Global monitor list, populated once in RdpAppProxy::resumed.
static MONITORS: OnceLock<Vec<MonitorInfo>> = OnceLock::new();

/// Populate the global monitor list from winit's ActiveEventLoop.
/// Called once from RdpAppProxy::resumed.
/// On Wayland, winit uses zxdg_output_v1 where available (wlroots, KDE, GNOME).
/// Falls back to empty if the compositor restricts screen info.
pub fn populate(event_loop: &ActiveEventLoop) {
    let _ = MONITORS.set(
        event_loop
            .available_monitors()
            .enumerate()
            .map(|(i, m): (usize, MonitorHandle)| MonitorInfo {
                index: i,
                size: (m.size().width, m.size().height),
                position: (m.position().x, m.position().y),
                scale: m.scale_factor(),
            })
            .collect(),
    );
}

/// Number of monitors detected at startup, or 1 as sensible default.
pub fn count() -> usize {
    MONITORS.get().map(|m| m.len()).unwrap_or(1)
}

/// Size of monitor `index`, or None if the index is out of bounds.
pub fn size(index: usize) -> Option<(u32, u32)> {
    MONITORS.get().and_then(|m| m.get(index)).map(|mi| mi.size)
}

/// Scale factor of monitor `index`, or 1.0 if not available.
pub fn scale(index: usize) -> f64 {
    MONITORS
        .get()
        .and_then(|m| m.get(index))
        .map(|mi| mi.scale)
        .unwrap_or(1.0)
}
