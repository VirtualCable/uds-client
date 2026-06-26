// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

mod crypt;
mod event;
mod executor;
mod jobs;
mod registry;
mod safe;
mod utils;

pub use crypt::crypt_protect_data;
pub use executor::execute_app;
pub use registry::{read_hkcu_str, read_hklm_str, write_hkcu_dword, write_hkcu_str};
