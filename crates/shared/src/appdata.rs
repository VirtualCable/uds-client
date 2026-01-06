// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

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

    // So we can override proxy and ssl settings if needed
    pub disable_proxy: Option<bool>,
    pub verify_ssl: Option<bool>,
    // On mac, also allow override launcher path
    #[cfg(target_os = "macos")]
    pub launcher_path: Option<String>,
}

impl AppData {
    
    pub fn load() -> Self {
        if let Some(proj_dirs) = ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_APPLICATION) {
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
