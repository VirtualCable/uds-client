// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

pub const UDS_CLIENT_VERSION: &str = "5.0.0";

// User-Agent string for HTTP requests, depends on OS
// to allow UDS to identify the client platform
#[cfg(target_os = "windows")]
pub const UDS_CLIENT_AGENT: &str = "UDS-Client/5.0.0 (Windows)";
#[cfg(target_os = "linux")]
pub const UDS_CLIENT_AGENT: &str = "UDS-Client/5.0.0 (Linux)";
#[cfg(target_os = "macos")]
pub const UDS_CLIENT_AGENT: &str = "UDS-Client/5.0.0 (MacOS)";
#[cfg(target_os = "android")]
pub const UDS_CLIENT_AGENT: &str = "UDS-Client/5.0.0 (Android)";

pub const URL_TEMPLATE: &str = "https://{host}/uds/rest/client";

pub const TICKET_LENGTH: usize = 48;
pub const MAX_STARTUP_TIME_MS: u64 = 120_000; // 2 minutes

// Keep-alive cadence towards the tunnel server. The server tears the
// launcher leg down after 10s of silence (KEEPALIVE_TIMEOUT_SECS on the
// server side); a fixed 2s period gives several chances to fit inside the
// deadline, and because the keep-alive rides the outbound stream's select
// loop it is naturally serialized with tunnel data (no cipher-seq
// contention) and only fires while the tunnel is idle.
pub const KEEPALIVE_INTERVAL_SECS: u64 = 2;

pub const LISTEN_ADDRESS: &str = "127.0.0.1";
pub const LISTEN_ADDRESS_V6: &str = "[::1]";
