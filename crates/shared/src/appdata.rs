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
#[serde(default)]
pub struct AppData {
    pub approved_hosts: Vec<String>,

    // SHA-256 fingerprints of server certificates the user has explicitly trusted
    pub trusted_certs: Vec<String>,

    // So we can override proxy and ssl settings if needed
    pub disable_proxy: Option<bool>,
    pub verify_ssl: Option<bool>,
    pub fps_limit: Option<u32>,
    // On mac, also allow override launcher path
    #[cfg(target_os = "macos")]
    pub launcher_path: Option<String>,
}

impl AppData {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_saved_by_previous_versions_is_still_readable() {
        let app_data: AppData =
            serde_json::from_str(r#"{"approved_hosts": ["some.host.com"]}"#).unwrap();

        assert_eq!(app_data.approved_hosts, vec!["some.host.com".to_string()]);
        assert!(app_data.trusted_certs.is_empty());
        assert_eq!(app_data.verify_ssl, None);
    }

    #[test]
    fn trusted_certs_survive_a_save_load_round_trip() {
        let app_data = AppData {
            trusted_certs: vec!["AA:BB:CC".to_string()],
            ..Default::default()
        };

        let restored: AppData =
            serde_json::from_str(&serde_json::to_string(&app_data).unwrap()).unwrap();

        assert_eq!(restored.trusted_certs, vec!["AA:BB:CC".to_string()]);
    }
}
