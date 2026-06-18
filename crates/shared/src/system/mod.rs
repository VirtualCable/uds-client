// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::{
    crypt_protect_data, execute_app, read_hkcu_str, read_hklm_str, write_hkcu_dword, write_hkcu_str,
};

#[cfg(not(target_os = "windows"))]
mod unix;
#[cfg(not(target_os = "windows"))]
pub use unix::execute_app;

pub mod trigger;

pub mod launcher;
