// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
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
//
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::fmt;
use zeroize::Zeroize;

use super::geom::ScreenSize;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize, Zeroize,
)]
#[serde(rename_all = "lowercase")]
pub enum WebcamCodec {
    #[default]
    Best,
    Fastest,
    Mjpeg,
    H264,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Zeroize)]
#[serde(default)]
pub struct WebcamSettings {
    pub enabled: bool,
    pub quality: u32,
    pub fps: u32,
    pub codec: WebcamCodec,
    /// Runtime: set before RDP connect from browser capabilities (query param ?h264=1)
    #[serde(skip)]
    pub browser_h264: bool,
    /// Camera width (from browser getUserMedia). Default 640.
    pub width: u32,
    /// Camera height (from browser getUserMedia). Default 480.
    pub height: u32,
    /// Optional size limit for output frames (width, height).
    pub size_limit: Option<(u32, u32)>,
}

impl Default for WebcamSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            quality: 80,
            fps: 15,
            codec: WebcamCodec::Best,
            browser_h264: false,
            width: 640,
            height: 480,
            size_limit: None,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Zeroize, Default,
)]
pub enum RailBehavior {
    CompositeGdi,
    #[default]
    IndividualWindows,
}

#[derive(Debug, Clone, Default, Zeroize)]
pub struct ServerInfo {
    #[zeroize(skip)]
    pub id: String,
    pub token: String,
}

#[derive(Debug, Clone, Zeroize, Default)]
pub struct RailSettings {
    pub app: String,
    pub args: Option<String>,
    pub working_dir: Option<String>,
    pub title: Option<String>,
    pub server_info: Option<ServerInfo>,
    pub behavior: RailBehavior,
}

#[derive(Debug, Clone, Zeroize)]
pub struct RdpRedirections {
    pub clipboard: bool,
    pub audio: bool,
    pub mic: bool,
    pub printing: bool,
    pub drives: Vec<String>,
    pub webcam: Option<WebcamSettings>,
    pub sound_latency_threshold: Option<u16>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Zeroize, Default,
)]
pub struct RdpFeatures {
    pub disable_threading: bool,
    pub force_software_gdi: bool,
}

#[derive(
    Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Zeroize,
)]
pub struct RdpOptions {
    pub use_nla: bool,
    pub verify_cert: bool,
    pub use_local_scaler: bool,
    pub use_tunnel: bool,
    pub desktop_scale: f64,
}

impl Default for RdpOptions {
    fn default() -> Self {
        Self {
            use_nla: true,
            verify_cert: false,
            use_local_scaler: true,
            use_tunnel: false,
            desktop_scale: 1.0,
        }
    }
}

#[derive(Clone, Zeroize)]
pub struct RdpSettings {
    pub server: String,
    pub port: u32,
    pub user: String,
    pub password: String,
    pub domain: String,
    pub screen_size: ScreenSize,
    pub best_experience: bool,

    pub redirections: RdpRedirections,
    pub rail: Option<RailSettings>,
    pub features: RdpFeatures,
    pub options: RdpOptions,
}

impl Default for RdpSettings {
    fn default() -> Self {
        RdpSettings {
            server: "".to_string(),
            port: 3389,
            user: "".to_string(),
            password: "".to_string(),
            domain: "".to_string(),
            screen_size: ScreenSize::Fixed(1024, 768),
            best_experience: true,
            redirections: RdpRedirections {
                clipboard: true,
                audio: true,
                mic: false,
                printing: false,
                drives: vec!["all".to_string()],
                webcam: None,
                sound_latency_threshold: None,
            },
            rail: None,
            features: RdpFeatures::default(),
            options: RdpOptions::default(),
        }
    }
}

// Debug without printing the password
impl fmt::Debug for RdpSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RdpSettings")
            .field("server", &self.server)
            .field("port", &self.port)
            .field("user", &self.user)
            .field("domain", &self.domain)
            .field("password", &{
                if self.password.is_empty() {
                    "\"\"".to_string()
                } else {
                    "\"****\"".to_string()
                }
            })
            .field("screen_size", &self.screen_size)
            .field("best_experience", &self.best_experience)
            .field("redirections", &self.redirections)
            .field("rail", &self.rail)
            .field("features", &self.features)
            .field("options", &self.options)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_port_is_3389() {
        assert_eq!(RdpSettings::default().port, 3389);
    }

    #[test]
    fn default_nla_is_true() {
        assert!(RdpSettings::default().options.use_nla);
    }

    #[test]
    fn default_screen_is_fixed_1024x768() {
        let s = RdpSettings::default();
        assert!(!s.screen_size.is_fullscreen());
        assert_eq!(s.screen_size.width(), 1024);
        assert_eq!(s.screen_size.height(), 768);
    }

    #[test]
    fn default_drives_redirect_all() {
        assert_eq!(RdpSettings::default().redirections.drives, vec!["all"]);
    }

    #[test]
    fn default_rail_none() {
        assert!(RdpSettings::default().rail.is_none());
    }

    #[test]
    fn debug_masks_password() {
        let s = RdpSettings {
            password: "secret".into(),
            ..Default::default()
        };
        let d = format!("{s:?}");
        assert!(!d.contains("secret"));
        assert!(d.contains("****"));
    }

    #[test]
    fn server_info_token_zeroizes() {
        let mut info = ServerInfo {
            id: "myid".into(),
            token: "mytok".into(),
        };
        info.zeroize();
        assert!(info.token.is_empty());
        assert!(!info.id.is_empty());
    }
}
