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

pub const URL_TEMPLATE: &str = "https://{host}/uds/rest/client";

pub const TICKET_LENGTH: usize = 48;
pub const MAX_STARTUP_TIME_MS: u64 = 120_000; // 2 minutes

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_length_is_48() {
        assert_eq!(TICKET_LENGTH, 48);
    }

    #[test]
    fn max_startup_time_is_2_minutes() {
        assert_eq!(MAX_STARTUP_TIME_MS, 120_000);
    }

    #[test]
    fn version_is_semver() {
        assert_eq!(UDS_CLIENT_VERSION, "5.0.0");
    }

    #[test]
    fn url_template_contains_host() {
        assert!(URL_TEMPLATE.contains("{host}"));
    }

    #[test]
    fn client_agent_is_non_empty() {
        assert!(!UDS_CLIENT_AGENT.is_empty());
    }
}
