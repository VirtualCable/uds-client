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

use crate::context::OwnerFromCtx;
use crate::utils;
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
            let _rects = region16_rects(&(*surface).invalidRegion, &mut nb_rects);
            if nb_rects == 0 {
                return CHANNEL_RC_OK;
            }

            let width = (*surface).width;
            let height = (*surface).height;
            let window_id = (*surface).windowId as u32;

            let mut mapped_width = (*surface).mappedWidth;
            let mut mapped_height = (*surface).mappedHeight;

            if mapped_width == 0 {
                mapped_width = width;
            }
            if mapped_height == 0 {
                mapped_height = height;
            }

            if mapped_width == 0 || mapped_height == 0 || (*surface).data.is_null() {
                return CHANNEL_RC_OK;
            }

            if mapped_width != width || mapped_height != height {
                log::trace!(
                    "GFX Surface size mismatch for window {}: surface={}x{}, mapped={}x{}",
                    window_id,
                    width,
                    height,
                    mapped_width,
                    mapped_height
                );
            }

            // Ensure we don't try to copy more than what we have in the surface
            // freerdp_image_copy doesn't scale, it just copies pixels.
            // If mapped size is different, it usually means scaling is handled elsewhere
            // or we should be using surface size for the copy.
            let copy_width = mapped_width.min(width);
            let copy_height = mapped_height.min(height);

            let mut data = vec![0u8; (mapped_width * mapped_height * 4) as usize];
            let format = if owner.use_rgba() {
                utils::pixel_format(32, 3, 8, 8, 8, 8) // RGBA32 (macOS)
            } else {
                utils::pixel_format(32, 4, 8, 8, 8, 8) // BGRA32 (Windows/Linux)
            };

            #[allow(clippy::unnecessary_cast)]
            // Needed beceuse windows/linux differ in the expected type of the flags parameter
            let _res = freerdp_image_copy(
                data.as_mut_ptr(),
                format,
                mapped_width * 4,
                0,
                0,
                copy_width,
                copy_height,
                (*surface).data,
                (*surface).format,
                (*surface).scanline,
                0,
                0,
                &(*gdi).palette,
                FREERDP_IMAGE_FLAGS_FREERDP_FLIP_NONE as u32,
            );

            // No swapping loop needed as we copy directly to the platform's native format

            if let Some(tx) = &owner.update_tx {
                // log::debug!("GFX sending WindowPixels for id={}, {}x{}", window_id, mapped_width, mapped_height);
                let _ = tx.try_send(crate::messaging::RdpMessage::WindowPixels {
                    window_id,
                    width: mapped_width,
                    height: mapped_height,
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
    pub unsafe fn hook_gdi(&self, gdi: *mut rdpGdi, use_individual_windows: bool) -> bool {
        if let Some(ptr) = &self.ptr {
            log::debug!("GFX: Hooking GDI pipeline");
            let context = ptr.as_mut_ptr();
            unsafe {
                if gdi_graphics_pipeline_init(gdi, context) != 0 {
                    log::info!("GFX: Graphics pipeline integrated with GDI.");
                    if use_individual_windows {
                        (*context).UpdateWindowFromSurface = Some(update_window_from_surface);
                    } else {
                        (*context).UpdateWindowFromSurface = None;
                    }
                    return true;
                }
            }
        }
        log::error!("GFX: Failed to hook GDI pipeline.");
        false
    }
}
