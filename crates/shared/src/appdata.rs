// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use directories_next::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::log;

const APP_DATA_FILE: &str = "app_data.json";
const APP_QUALIFIER: &str = "org";
const APP_ORGANIZATION: &str = "openuds";
const APP_APPLICATION: &str = "launcher";

#[derive(Serialize, Deserialize, Default)]
pub struct AppData {
    pub approved_hosts: Vec<String>,

    #[serde(default)]
    pub insecure_allowed_hosts: Vec<String>,

    // So we can override proxy settings if needed
    pub disable_proxy: Option<bool>,
    pub fps_limit: Option<u32>,
    // On mac, also allow override launcher path
    #[cfg(target_os = "macos")]
    pub launcher_path: Option<String>,
}

impl AppData {
    pub fn verify_ssl(&self, hostname: &str) -> bool {
        let hostname = hostname_without_port(hostname);
        !self
            .insecure_allowed_hosts
            .iter()
            .any(|allowed_host| hostname_without_port(allowed_host).eq_ignore_ascii_case(hostname))
    }

    pub fn load() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_APPLICATION)
        {
            let data_dir = proj_dirs.data_dir();
            let file_path = data_dir.join(APP_DATA_FILE);
            log::debug!("Loading app data from {:?}", file_path);
            if let Ok(data) = std::fs::read_to_string(file_path)
                && let Ok(app_data) = serde_json::from_str(&data)
            {
                return app_data;
            }
        }

        Self::default()
    }

    pub fn save(&self) {
        if let Some(proj_dirs) = ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_APPLICATION)
        {
            let data_dir = proj_dirs.data_dir();
            if let Err(e) = std::fs::create_dir_all(data_dir) {
                log::error!("Failed to create data directory: {}", e);
                return;
            }
            let file_path = data_dir.join(APP_DATA_FILE);
            match serde_json::to_string_pretty(self) {
                Ok(data) => {
                    if let Err(e) = std::fs::write(&file_path, data) {
                        log::error!("Failed to write app data to {:?}: {}", file_path, e);
                    }
                }
                Err(e) => {
                    log::error!("Failed to serialize app data: {}", e);
                }
            }
        }
    }
}

fn hostname_without_port(hostname: &str) -> &str {
    if let Some(hostname) = hostname.strip_prefix('[') {
        return hostname.split_once(']').map_or(hostname, |(host, _)| host);
    }

    if hostname.matches(':').count() == 1 {
        return hostname.split_once(':').map_or(hostname, |(host, _)| host);
    }

    hostname
}

#[cfg(test)]
mod tests {
    use super::AppData;

    #[test]
    fn verifies_ssl_for_hosts_not_in_allowlist() {
        let app_data = AppData {
            insecure_allowed_hosts: vec!["self-signed.example.com".to_string()],
            ..Default::default()
        };

        assert!(!app_data.verify_ssl("self-signed.example.com"));
        assert!(!app_data.verify_ssl("SELF-SIGNED.EXAMPLE.COM"));
        assert!(app_data.verify_ssl("trusted.example.com"));
    }

    #[test]
    fn allowlist_requires_exact_hostname_match() {
        let app_data = AppData {
            insecure_allowed_hosts: vec!["example.com".to_string()],
            ..Default::default()
        };

        assert!(app_data.verify_ssl("sub.example.com"));
        assert!(!app_data.verify_ssl("example.com:443"));
    }

    #[test]
    fn allowlist_compares_ipv6_hostname_without_port() {
        let app_data = AppData {
            insecure_allowed_hosts: vec!["2001:db8::1".to_string()],
            ..Default::default()
        };

        assert!(!app_data.verify_ssl("[2001:DB8::1]:443"));
    }
}
