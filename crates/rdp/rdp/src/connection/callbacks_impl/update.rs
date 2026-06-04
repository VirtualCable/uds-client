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

use freerdp_sys::*;

use shared::log;

use crate::callbacks::update;

use super::{Rdp, RdpMessage};

impl update::UpdateCallbacks for Rdp {
    fn on_begin_paint(&mut self) -> bool {
        true
    }

    fn on_end_paint(&mut self) -> bool {
        log::trace!("on_end_paint called");
        self.send_update()
    }

    fn on_desktop_resize(&mut self) -> bool {
        log::debug!(" **** Desktop resized");
        if let Some(settings) = self.settings() {
            let width = unsafe {
                freerdp_settings_get_uint32(
                    settings,
                    FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopWidth,
                )
            };
            let height = unsafe {
                freerdp_settings_get_uint32(
                    settings,
                    FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopHeight,
                )
            };
            let gdi_lock = self.gdi_lock();
            let _gdi_guard = gdi_lock.write().unwrap();
            if let Some(gdi) = self.gdi() {
                unsafe { gdi_resize(gdi, width as u32, height as u32) };
            }
            // If update_tx is present, notify it of the resize
            // with try_send, so it doesn't block if the gui thread is not ready
            if let Some(tx) = &self.update_tx {
                let _ = tx.try_send(RdpMessage::DesktopResize(width as u32, height as u32));
            }
            true
        } else {
            log::debug!("No settings found");
            false
        }
    }
}

impl Rdp {
    fn send_update(&self) -> bool {
        log::trace!("send_update called");
        if let Some(tx) = &self.update_tx
            && let Some(gdi) = self.gdi()
        {
            unsafe {
                // CRITICAL: Use gdi->primary->hdc->hwnd->invalid,
                // NOT gdi->drawing. The GFX pipeline writes to primary and sets
                // its invalidation region, but 'drawing' may point elsewhere.
                let primary = (*gdi).primary;
                if primary.is_null() {
                    return true;
                }
                let hdc = (*primary).hdc;
                if hdc.is_null() || (*hdc).hwnd.is_null() {
                    return true;
                }

                let hwnd = (*hdc).hwnd;
                let rgn = (*hwnd).invalid;
                let mut ninvalid = (*hwnd).ninvalid;

                // Sanity check: limit the number of invalid rectangles to prevent DoS/OOB
                if ninvalid > 256 {
                    log::warn!(
                        "RAIL: Too many invalid rectangles: {}, capping to 256",
                        ninvalid
                    );
                    ninvalid = 256;
                }

                #[allow(clippy::unnecessary_cast)]
                // Needed beceuse windows/linux differ in the expected type of the flags parameter
                if !rgn.is_null() && ((*rgn).null == 0 || ninvalid > 0) {
                    let mut rects = vec![];
                    if (*rgn).null == 0 {
                        let r = &*rgn;
                        // Basic dimension check
                        if r.w > 0 && r.h > 0 && r.w < 16384 && r.h < 16384 {
                            rects.push(crate::geom::Rect::new(
                                r.x as i32, r.y as i32, r.w as u32, r.h as u32,
                            ));
                        }
                    }
                    if ninvalid > 0 {
                        let cinvalid = (*hwnd).cinvalid;
                        if !cinvalid.is_null() {
                            let slice = std::slice::from_raw_parts(cinvalid, ninvalid as usize);
                            for crgn in slice.iter() {
                                if crgn.null == 0
                                    && crgn.w > 0
                                    && crgn.h > 0
                                    && crgn.w < 16384
                                    && crgn.h < 16384
                                {
                                    rects.push(crate::geom::Rect::new(
                                        crgn.x as i32,
                                        crgn.y as i32,
                                        crgn.w as u32,
                                        crgn.h as u32,
                                    ));
                                }
                            }
                        }
                    }

                    if !rects.is_empty() {
                        // Use trace instead of debug
                        log::trace!("Sending UpdateRects: block, items: {}", rects.len());
                        let _ = tx.try_send(RdpMessage::UpdateRects(rects));
                    }

                    // Reset invalidation after sending, following Guacamole's pattern
                    (*rgn).null = 1;
                    (*hwnd).ninvalid = 0;
                }
            }
        }
        true
    }
}
