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
//
// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::fmt;
use zeroize::Zeroize;

use super::geom::ScreenSize;

#[derive(Zeroize, Clone)]
pub struct RdpSettings {
    #[zeroize(skip)]
    pub server: String,
    #[zeroize(skip)]
    pub port: u32,
    pub user: String,
    pub password: String,
    pub domain: String,
    #[zeroize(skip)]
    pub verify_cert: bool,
    #[zeroize(skip)]
    pub use_nla: bool,
    #[zeroize(skip)]
    pub screen_size: ScreenSize,
    #[zeroize(skip)]
    pub clipboard_redirection: bool,
    #[zeroize(skip)]
    pub audio_redirection: bool,
    #[zeroize(skip)]
    pub microphone_redirection: bool,
    #[zeroize(skip)]
    pub printer_redirection: bool,
    // Valid values for drives_to_redirect are "all" for all drives
    // % -> Home
    // * --> All drives
    // DynamicDrives --> Later connected drives
    #[zeroize(skip)]
    pub drives_to_redirect: Vec<String>,
    #[zeroize(skip)]
    pub sound_latency_threshold: Option<u16>,
    #[zeroize(skip)]
    pub best_experience: bool,
}

impl Default for RdpSettings {
    fn default() -> Self {
        RdpSettings {
            server: "".to_string(),
            port: 3389,
            user: "".to_string(),
            password: "".to_string(),
            domain: "".to_string(),
            verify_cert: false,
            use_nla: true, // Defaults to true for better security, but can be disabled if needed
            screen_size: ScreenSize::Fixed(1024, 768),
            clipboard_redirection: true,
            audio_redirection: true,
            microphone_redirection: false,
            printer_redirection: false,
            drives_to_redirect: vec!["all".to_string()], // By default, redirect all drives.
            sound_latency_threshold: None,
            best_experience: true,
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
            .field("verify_cert", &self.verify_cert)
            .field("use_nla", &self.use_nla)
            .field("screen_size", &self.screen_size)
            .field("clipboard_redirection", &self.clipboard_redirection)
            .field("audio_redirection", &self.audio_redirection)
            .field("microphone_redirection", &self.microphone_redirection)
            .field("printer_redirection", &self.printer_redirection)
            .field("drives_to_redirect", &self.drives_to_redirect)
            .field("sound_latency_threshold", &self.sound_latency_threshold)
            .field("best_experience", &self.best_experience)
            .finish()
    }
}
