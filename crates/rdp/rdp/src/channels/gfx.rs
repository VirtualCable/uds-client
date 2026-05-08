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

use crate::utils;
use freerdp_sys::*;
use shared::log;

#[derive(Clone, Debug)]
pub struct GfxChannel {
    ptr: Option<utils::SafePtr<freerdp_sys::RdpgfxClientContext>>,
}

impl GfxChannel {
    pub fn new(ptr: *mut freerdp_sys::RdpgfxClientContext) -> Self {
        Self {
            ptr: utils::SafePtr::new(ptr),
        }
    }

    /// # Safety
    ///
    /// The caller must ensure that `gdi` is a valid pointer to an `rdpGdi` structure.
    /// The caller must also ensure that the `GfxChannel` is valid and that the
    /// `rdpGdi` structure is properly initialized.
    ///
    /// Hooks the Graphics Pipeline into the GDI drawing engine.
    /// This handles drawing GFX frames into the GDI surface and
    /// automatically sends frame acknowledgments for flow control.
    pub unsafe fn hook_gdi(&self, gdi: *mut rdpGdi) -> bool {
        if let Some(ptr) = &self.ptr {
            log::debug!("GFX: Hooking GDI pipeline");
            let context = ptr.as_mut_ptr();
            unsafe {
                if gdi_graphics_pipeline_init(gdi, context) != 0 {
                    log::info!("GFX: Graphics pipeline integrated with GDI.");
                    return true;
                }
            }
        }
        log::error!("GFX: Failed to hook GDI pipeline.");
        false
    }
}
