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
use freerdp_sys::*;

use shared::log;

use crate::{callbacks::update, utils::normalize_rects};

use super::{Rdp, RdpMessage};

impl update::UpdateCallbacks for Rdp {
    fn on_begin_paint(&mut self) -> bool {
        // Note: Regions are cleared by update_begin_paint by FreeRDP itself
        // Else, we should have to set invalid.null to true and ninvalid to 0 here manually on hwnd.

        true
    }

    fn on_end_paint(&mut self) -> bool {
        // If no sender, skip
        if let Some(tx) = &self.update_tx {
            // If no updates, skip
            if let Some(gdi) = self.gdi() {
                // We can simply get "invalid", that is the joined rects that needs update
                // for more granular updates, we get all rects and send them individually
                let (rects_raw, width, height) = unsafe {
                    let primary = &mut *(*gdi).primary;
                    let width = (*gdi).width as u32;
                    let height = (*gdi).height as u32;
                    let hwnd = (*primary.hdc).hwnd;
                    if (*hwnd).invalid.is_null()
                        || (*(*hwnd).invalid).null != 0
                        || (*hwnd).ninvalid <= 0
                    {
                        return true;
                    }

                    // Currently, using joined rect only (invalid), individials comes on cinvalid with ninvalid count
                    // But this should be enough for most cases (until implemented our own drawing routines)
                    (
                        std::slice::from_raw_parts((*hwnd).invalid, 1),
                        width,
                        height,
                    )
                };

                if let Some(rects) = normalize_rects(rects_raw, width, height) {
                    let _ = tx.try_send(RdpMessage::UpdateRects(rects));
                }
            }
        }
        true
    }

    fn on_desktop_resize(&mut self) -> bool {
        log::debug!(" **** Desktop resized");
        let width = unsafe {
            freerdp_settings_get_uint32(
                self.context().unwrap().context().settings,
                FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopWidth,
            )
        };
        let height = unsafe {
            freerdp_settings_get_uint32(
                self.context().unwrap().context().settings,
                FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopHeight,
            )
        };
        let gdi_lock = self.gdi_lock();
        let _gdi_guard = gdi_lock.write().unwrap();
        if let Some(gdi) = self.gdi() {
            unsafe { gdi_resize(gdi, width as u32, height as u32) };
        }
        true
    }
}
