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

use std::ffi::CString;

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use shared::log;

use crate::callbacks::instance;
use freerdp_sys::{
    FreeRDP_Settings_Keys_String_FreeRDP_ServerHostname, freerdp_settings_set_string,
};

use super::Rdp;

// Sombra de S_H264_CONTEXT de FreeRDP para acceder a NumberOfThreads
// ya que el header h264.h no es público.
#[repr(C)]
struct H264ContextShadow {
    _compressor: i32,
    _width: u32,
    _height: u32,
    _rate_control_mode: u32,
    _bit_rate: u32,
    _frame_rate: u32,
    _qp: u32,
    _usage_type: u32,
    _hw_accel: u32,
    pub num_threads: u32,
}

impl instance::InstanceCallbacks for Rdp {
    fn on_post_connect(&mut self) -> bool {
        log::debug!(" **** Connected successfully!");

        // Limit FFmpeg threads to avoid the thread-per-core explosion
        if let Some(instance) = &self.instance {
            unsafe {
                let context = instance.context;
                if !context.is_null() && !(*context).codecs.is_null() {
                    let codecs = (*context).codecs;
                    let h264 = (*codecs).h264;
                    if !h264.is_null() {
                        log::debug!("Limiting FFmpeg decoder threads to 2 (via Shadow Struct)");
                        let h264_shadow = h264 as *mut H264ContextShadow;
                        (*h264_shadow).num_threads = 2;
                    }
                }
            }
        }
        true
    }

    fn on_redirect(&mut self) -> bool {
        log::debug!(" **** Redirecting!");
        // Override FreeRDP_ServerHostname with original hostname if tunnel flag is set
        if self.config.settings.options.use_tunnel
            && let Some(settings) = self.settings()
            && let Ok(host) = CString::new(self.config.settings.server.as_str())
        {
            log::debug!("Override FreeRDP_ServerHostname with original");
            unsafe {
                freerdp_settings_set_string(
                    settings,
                    FreeRDP_Settings_Keys_String_FreeRDP_ServerHostname,
                    host.as_ptr(),
                );
            }
        };
        true
    }
}
