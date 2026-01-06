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
use std::sync::OnceLock;

use freerdp_sys::{
    BOOL, DWORD, FREERDP_LOAD_CHANNEL_ADDIN_ENTRY_FN, LPCSTR, PVIRTUALCHANNELENTRY,
    RDPSND_CHANNEL_NAME, UINT, freerdp_get_current_addin_provider, freerdp_register_addin_provider,
};

use super::audio_output;
use shared::log;

static FREERDP_ADDIN_PROVIDER: OnceLock<FREERDP_LOAD_CHANNEL_ADDIN_ENTRY_FN> = OnceLock::new();

fn secure_cstr_from_lpcstr(psz: LPCSTR) -> String {
    if psz.is_null() {
        "<null>".to_string()
    } else {
        unsafe {
            std::ffi::CStr::from_ptr(psz)
                .to_str()
                .unwrap_or("<invalid utf8>")
                .to_string()
        }
    }
}

unsafe extern "C" fn custom_addin_provider(
    psz_name: LPCSTR,
    psz_subsystem: LPCSTR,
    psz_type: LPCSTR,
    dw_flags: DWORD,
) -> PVIRTUALCHANNELENTRY {
    unsafe {
        let name = secure_cstr_from_lpcstr(psz_name);
        let subsystem = secure_cstr_from_lpcstr(psz_subsystem);

        if let Some(freerdp_addin_provider) = FREERDP_ADDIN_PROVIDER.get().unwrap() {
            if name.as_bytes() == &RDPSND_CHANNEL_NAME[..RDPSND_CHANNEL_NAME.len() - 1]
                && subsystem == super::RDPSND_SUBSYSTEM_CUSTOM
            {
                log::info!("rdpsnd channel addin requested.");
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(
                        *mut freerdp_sys::FREERDP_RDPSND_DEVICE_ENTRY_POINTS,
                    ) -> UINT,
                    unsafe extern "C" fn(*mut freerdp_sys::tagCHANNEL_ENTRY_POINTS) -> BOOL,
                >(audio_output::sound_entry))
            } else {
                freerdp_addin_provider(psz_name, psz_subsystem, psz_type, dw_flags)
            }
        } else {
            log::error!("No underlying addin provider found.");
            None
        }
    }
}

pub fn register_channel_addin_loader() {
    FREERDP_ADDIN_PROVIDER
        .set(unsafe { freerdp_get_current_addin_provider() })
        .ok();
    unsafe {
        freerdp_register_addin_provider(Some(custom_addin_provider), 0);
    }
}
