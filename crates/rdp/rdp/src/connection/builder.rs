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
use std::ffi::CString;

use anyhow::Result;
use freerdp_sys::*;

use crate::{callbacks::instance_c, utils::SafePtr};
use shared::log;

use super::{Rdp, context::RdpContext};

#[allow(dead_code)]
impl Rdp {
    pub fn build(self: std::pin::Pin<&mut Self>) -> Result<()> {
        log::debug!("Building RDP connection... {:p}", self);
        let mut_self = unsafe { self.get_unchecked_mut() };

        unsafe {
            let ctx = RdpContext::create(mut_self)?;
            let instance = (*ctx).common.context.instance;

            mut_self.instance = Some(SafePtr::new(instance).ok_or_else(|| {
                anyhow::anyhow!(
                    "Failed to create SafePtr for freerdp instance: {:?}",
                    instance
                )
            })?);

            instance_c::set_instance_callbacks(instance);

            let settings_ptr = (*ctx).common.context.settings;

            let host = CString::new(mut_self.config.settings.server.clone())?;
            let user = CString::new(mut_self.config.settings.user.clone())?;
            let pass = CString::new(mut_self.config.settings.password.clone())?;
            let domain = CString::new(mut_self.config.settings.domain.clone())?;
            let drives_to_redirect = CString::new(
                mut_self
                    .config
                    .settings
                    .drives_to_redirect
                    .iter()
                    .map(|s| match s.as_str() {
                        "all" => "*",
                        "DynamicDrives" => "DynamicDrives",
                        other => other,
                    })
                    .collect::<Vec<&str>>()
                    .join(";"),
            )
            .unwrap();

            freerdp_settings_set_string(
                settings_ptr,
                FreeRDP_Settings_Keys_String_FreeRDP_ServerHostname,
                host.as_ptr(),
            );
            freerdp_settings_set_string(
                settings_ptr,
                FreeRDP_Settings_Keys_String_FreeRDP_Username,
                user.as_ptr(),
            );
            freerdp_settings_set_string(
                settings_ptr,
                FreeRDP_Settings_Keys_String_FreeRDP_Password,
                pass.as_ptr(),
            );
            freerdp_settings_set_string(
                settings_ptr,
                FreeRDP_Settings_Keys_String_FreeRDP_Domain,
                domain.as_ptr(),
            );
            freerdp_settings_set_uint32(
                settings_ptr,
                FreeRDP_Settings_Keys_UInt32_FreeRDP_ServerPort,
                mut_self.config.settings.port,
            );
            freerdp_settings_set_bool(
                settings_ptr,
                FreeRDP_Settings_Keys_Bool_FreeRDP_IgnoreCertificate,
                !mut_self.config.settings.verify_cert as BOOL,
            );

            // NLA setting
            freerdp_settings_set_bool(
                settings_ptr,
                FreeRDP_Settings_Keys_Bool_FreeRDP_NlaSecurity,
                mut_self.config.settings.use_nla as BOOL,
            );

            let all_drives = mut_self
                .config
                .settings
                .drives_to_redirect
                .iter()
                .any(|s| s.as_str() == "all");
            let len_drives = mut_self.config.settings.drives_to_redirect.len();
            freerdp_settings_set_bool(
                settings_ptr,
                FreeRDP_Settings_Keys_Bool_FreeRDP_RedirectDrives,
                (len_drives != 0) as BOOL,
            );
            if !all_drives {
                // Remove "all" and, if any rameaining, use FreeRDP_RedirectDrives
                freerdp_settings_set_string(
                    settings_ptr,
                    FreeRDP_Settings_Keys_String_FreeRDP_DrivesToRedirect,
                    drives_to_redirect.as_ptr(),
                );
            }

            Ok(())
        }
    }
}
