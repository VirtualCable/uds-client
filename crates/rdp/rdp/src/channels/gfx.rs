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
use crate::context::OwnerFromCtx;
use freerdp_sys::*;
use shared::log;

#[derive(Clone, Debug)]
pub struct GfxChannel {
    ptr: Option<utils::SafePtr<freerdp_sys::RdpgfxClientContext>>,
}

extern "C" fn update_window_from_surface(
    context: *mut RdpgfxClientContext,
    surface: *mut gdiGfxSurface,
) -> UINT {
    unsafe {
        let gdi = (*context).custom as *mut rdpGdi;
        let rdp_context = (*gdi).context;
        if let Some(owner) = rdp_context.owner() {
            let mut nb_rects = 0;
            let _rects = region16_rects(&mut (*surface).invalidRegion, &mut nb_rects);
            if nb_rects == 0 {
                return CHANNEL_RC_OK;
            }

            let width = (*surface).width;
            let height = (*surface).height;
            let window_id = (*surface).windowId as u32;

            if width == 0 || height == 0 || (*surface).data.is_null() {
                return CHANNEL_RC_OK;
            }

            let mut data = vec![0u8; (width * height * 4) as usize];
            let format = if owner.use_rgba() {
                utils::pixel_format(32, 3, 8, 8, 8, 8)
            } else {
                utils::pixel_format(32, 4, 8, 8, 8, 8)
            };

            let res = freerdp_image_copy(
                data.as_mut_ptr(),
                format,
                width * 4,
                0,
                0,
                width,
                height,
                (*surface).data,
                (*surface).format,
                (*surface).scanline,
                0,
                0,
                &(*gdi).palette,
                FREERDP_IMAGE_FLAGS_FREERDP_FLIP_NONE,
            );

            if res == 0 {
                log::error!("freerdp_image_copy failed!");
            }

            // Re-order BGRA to RGBA if necessary, freerdp_image_copy might not do everything perfectly for egui
            if !owner.use_rgba() {
                for chunk in data.chunks_exact_mut(4) {
                    chunk.swap(0, 2); // Swap B and R
                }
            }

            if let Some(tx) = &owner.update_tx {
                // log::debug!("GFX sending WindowPixels for id={}, {}x{}", window_id, width, height);
                let _ = tx.try_send(crate::messaging::RdpMessage::WindowPixels {
                    window_id,
                    width,
                    height,
                    data,
                });
            }

            // Clear invalid region
            region16_clear(&mut (*surface).invalidRegion);
        }
        CHANNEL_RC_OK
    }
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
                    (*context).UpdateWindowFromSurface = Some(update_window_from_surface);
                    return true;
                }
            }
        }
        log::error!("GFX: Failed to hook GDI pipeline.");
        false
    }
}
