// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use crypt::types::{SharedSecret, Ticket};

pub struct TunnelConnectInfo {
    pub addr: String,
    pub port: u16,
    pub ticket: Ticket,
    pub local_port: Option<u16>, // If None, a random port will be used
    pub check_certificate: bool, // whether to check server certificate, v4.0
    pub startup_time_ms: u64,    // Timeout for listening, in milliseconds
    pub keep_listening_after_timeout: bool, // whether to keep listening after timeout
    pub enable_ipv6: bool,       // whether to enable ipv6 (local and remote)
    pub shared_secret: Option<SharedSecret>, // cryptographic keys for the connection. v5.0
}
