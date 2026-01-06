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
use shared::log;

use crate::callbacks::graphics;

use super::{Rdp, RdpMessage};

impl graphics::GraphicsCallbacks for Rdp {
    unsafe fn on_pointer_set(&self, pointer: *mut freerdp_sys::rdpPointer) -> bool {
        let pointer = unsafe { &*pointer };
        let gdi = match self.gdi() {
            Some(gdi) => gdi,
            None => {
                log::error!(" **** GDI context not available.");
                return false;
            }
        };
        let size = 4 * pointer.width * pointer.height;
        let data = vec![0u8; size as usize];
        // Create the custom pointer image from the pointer data
        unsafe {
            freerdp_sys::freerdp_image_copy_from_pointer_data(
                data.as_ptr() as *mut freerdp_sys::BYTE,
                (*gdi).dstFormat,
                0,
                0,
                0,
                pointer.width,
                pointer.height,
                pointer.xorMaskData,
                pointer.lengthXorMask,
                pointer.andMaskData,
                pointer.lengthAndMask,
                pointer.xorBpp,
                &(*gdi).palette,
            )
        };
        // Send the custom pointer data to the UI or handle it as needed
        if let Some(tx) = &self.update_tx
            && let Err(e) = tx.try_send(RdpMessage::SetCursorIcon(
                data,
                pointer.xPos,
                pointer.yPos,
                pointer.width,
                pointer.height,
            ))
        {
            log::error!(" **** Failed to send custom pointer data: {}", e);
        }
        true
    }

    unsafe fn on_pointer_free(&self, _pointer: *mut freerdp_sys::rdpPointer) {
        // We do not need special handling for freeing the pointer in this implementation.
        // Because the cursor data was sent to the UI.
    }

    unsafe fn on_pointer_new(&self, _pointer: *mut freerdp_sys::rdpPointer) -> bool {
        // We do not need special handling for new pointers in this implementation.
        // Because the cursor data will be sent to the UI when set.
        true
    }

    fn on_pointer_position(&self, _x: u32, _y: u32) -> bool {
        // We do not need special handling for pointer position in this implementation.
        // Because the cursor position will be handled by the UI.
        true
    }

    fn on_pointer_set_null(&self) -> bool {
        if let Some(tx) = &self.update_tx
            && let Err(e) = tx.try_send(RdpMessage::SetCursorIcon(vec![0u8; 4], 0, 0, 1, 1))
        {
            log::error!(" **** Failed to send null pointer data: {}", e);
        }
        true
    }
}
