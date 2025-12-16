// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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
#![allow(unused_assignments)]
use zeroize::Zeroize;

use super::geom::ScreenSize;

#[derive(Zeroize, Debug, Clone)]
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
            use_nla: false,
            screen_size: ScreenSize::Fixed(1024, 768),
            clipboard_redirection: true,
            audio_redirection: true,
            microphone_redirection: false,
            printer_redirection: false,
            drives_to_redirect: vec!["all".to_string()], // By default, redirect all drives.
            sound_latency_threshold: None,
        }
    }
}
