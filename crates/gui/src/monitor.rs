// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
// Monitor geometry, queried from winit at startup via RdpAppProxy::resumed.
use std::sync::{LazyLock, OnceLock};

use winit::{event_loop::ActiveEventLoop, monitor::MonitorHandle};

#[allow(dead_code)]
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

#[allow(dead_code)]
/// Number of monitors detected at startup, or 1 as sensible default.
pub fn count() -> usize {
    MONITORS.get().map(|m| m.len()).unwrap_or(1)
}

#[allow(dead_code)]
/// Size of monitor `index`, or None if the index is out of bounds.
pub fn size(index: usize) -> Option<(u32, u32)> {
    MONITORS.get().and_then(|m| m.get(index)).map(|mi| mi.size)
}

#[allow(dead_code)]
/// Scale factor of monitor `index`, or 1.0 if not available.
pub fn scale(index: usize) -> f64 {
    MONITORS
        .get()
        .and_then(|m| m.get(index))
        .map(|mi| mi.scale)
        .unwrap_or(1.0)
}

/// Cached scale factor from monitor 0. Populated after `populate()` is called.
pub static SCALE_FACTOR: LazyLock<f64> = LazyLock::new(|| scale(0));

/// DPI-scaled integer value (logical → physical using cached scale factor).
pub fn scaled_val(val: i32) -> i32 {
    (val as f64 * *SCALE_FACTOR).round() as i32
}

/// Convert logical (GDI) pixel pair to physical (screen) pixels.
pub fn logic_2_phys_pos(logical: (i32, i32), sf: f64) -> (i32, i32) {
    (
        (logical.0 as f64 * sf).round() as i32,
        (logical.1 as f64 * sf).round() as i32,
    )
}

/// Convert physical (screen) pixel pair to logical (GDI) pixels.
pub fn phys_2_logic(physical: (i32, i32), sf: f64) -> (i32, i32) {
    (
        (physical.0 as f64 / sf).round() as i32,
        (physical.1 as f64 / sf).round() as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phys_to_logic_identity() {
        assert_eq!(phys_2_logic((100, 200), 1.0), (100, 200));
    }

    #[test]
    fn phys_to_logic_scale_2() {
        assert_eq!(phys_2_logic((100, 50), 2.0), (50, 25));
    }

    #[test]
    fn phys_to_logic_rounding() {
        // 5 / 2.0 = 2.5, round = 3
        assert_eq!(phys_2_logic((5, 5), 2.0), (3, 3));
        // 4 / 2.0 = 2.0, exact
        assert_eq!(phys_2_logic((4, 4), 2.0), (2, 2));
    }

    #[test]
    fn logic_to_phys_identity() {
        assert_eq!(logic_2_phys_pos((50, 100), 1.0), (50, 100));
    }

    #[test]
    fn logic_to_phys_scale_2() {
        assert_eq!(logic_2_phys_pos((50, 25), 2.0), (100, 50));
    }

    #[test]
    fn logic_to_phys_rounding() {
        // 3 * 1.5 = 4.5, round = 5
        assert_eq!(logic_2_phys_pos((3, 3), 1.5), (5, 5));
    }

    #[test]
    fn round_trip() {
        let phys = (3840, 2160);
        let sf = 2.0;
        let logical = phys_2_logic(phys, sf);
        let back = logic_2_phys_pos(logical, sf);
        // May differ by 1 due to rounding
        assert!((back.0 - phys.0).abs() <= 1);
        assert!((back.1 - phys.1).abs() <= 1);
    }
}
