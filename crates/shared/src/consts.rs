pub const UDS_CLIENT_VERSION: &str = "5.0.0";

// User-Agent string for HTTP requests, depends on OS
// to allow UDS to identify the client platform
#[cfg(target_os = "windows")]
pub const UDS_CLIENT_AGENT: &str = "UDS-Client/5.0.0 (Windows)";
#[cfg(target_os = "linux")]
pub const UDS_CLIENT_AGENT: &str = "UDS-Client/5.0.0 (Linux)";
#[cfg(target_os = "macos")]
pub const UDS_CLIENT_AGENT: &str = "UDS-Client/5.0.0 (MacOS)";

pub const URL_TEMPLATE: &str = "https://{host}/uds/rest/client";

pub const TICKET_LENGTH: usize = 48;
pub const MAX_STARTUP_TIME_MS: u64 = 120_000; // 2 minutes