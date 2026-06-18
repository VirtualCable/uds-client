// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use winit::keyboard::KeyCode;

mod scancodes;
mod winit_keys;

pub use scancodes::RdpScanCode;

impl RdpScanCode {
    pub fn get_from_key(key: Option<&KeyCode>) -> Option<Self> {
        if let Some(k) = key {
            RdpScanCode::from_winit_key(k)
        } else {
            None
        }
    }
}
