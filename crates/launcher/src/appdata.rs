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

    // So we can override proxy and ssl settings if needed
    pub disable_proxy: Option<bool>,
    pub verify_ssl: Option<bool>,
}

impl AppData {
    
    pub fn load() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_APPLICATION) {
            let data_dir = proj_dirs.data_dir();
            let file_path = data_dir.join(APP_DATA_FILE);
            if let Ok(data) = std::fs::read_to_string(file_path)
                && let Ok(app_data) = serde_json::from_str(&data)
            {
                return app_data;
            }
        }

        Self::default()
    }

    pub fn save(&self) {
        if let Some(proj_dirs) = ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_APPLICATION) {
            let data_dir = proj_dirs.data_dir();
            if let Err(e) = std::fs::create_dir_all(data_dir) {
                log::error!("Failed to create data directory: {}", e);
                return;
            }
            let file_path = data_dir.join(APP_DATA_FILE);
            if let Ok(data) = serde_json::to_string_pretty(self)
                && let Err(e) = std::fs::write(file_path, data)
            {
                log::error!("Failed to write app data: {}", e);
            } else {
                log::error!("Failed to serialize app data");
            }
        }
    }
}
