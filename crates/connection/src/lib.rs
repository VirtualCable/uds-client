// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

pub mod v4;
pub mod v5;

pub mod broker;
pub mod consts;
pub mod registry;
pub mod tasks;
pub mod types;

mod tunnel;
mod utils;

pub use tunnel::start_tunnel;

// Re-export CryptoConfig from crypt crate
pub use crypt::secrets::CryptoKeys;
