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

use crate::geom::Rect;
use crate::utils::log;

#[derive(Clone, Debug)]
pub struct DispChannel {
    ptr: Option<utils::SafePtr<freerdp_sys::DispClientContext>>,
}

impl DispChannel {
    pub fn new(ptr: *mut freerdp_sys::DispClientContext) -> Self {
        Self {
            ptr: utils::SafePtr::new(ptr),
        }
    }

    // Only implemented what used
    pub fn send_monitor_layout(
        &self,
        rect: Rect,
        orientation: u32,
        desktop_scale_factor: u32,
        device_scale_factor: u32,
    ) {
        log::debug!(
            "Sending monitor layout: {:?}, scale: {}/{}",
            rect,
            desktop_scale_factor,
            device_scale_factor
        );
        if let Some(ptr) = &self.ptr {
            let dcml = freerdp_sys::DISPLAY_CONTROL_MONITOR_LAYOUT {
                Flags: freerdp_sys::DISPLAY_CONTROL_MONITOR_PRIMARY,
                Left: rect.x as freerdp_sys::INT32,
                Top: rect.y as freerdp_sys::INT32,
                Width: rect.w,
                Height: rect.h,
                Orientation: orientation as freerdp_sys::UINT32,
                DesktopScaleFactor: desktop_scale_factor,
                DeviceScaleFactor: device_scale_factor,
                PhysicalWidth: rect.w,
                PhysicalHeight: rect.h,
            };
            let mut dcml_vec = vec![dcml];
            // call calback
            if let Some(func) = ptr.SendMonitorLayout {
                log::debug!("Calling SendMonitorLayout callback...");
                let result = unsafe {
                    func(
                        ptr.as_mut_ptr(),
                        dcml_vec.len() as freerdp_sys::UINT32,
                        dcml_vec.as_mut_ptr(),
                    )
                };
                log::debug!("SendMonitorLayout result: {}", result);
            } else {
                log::warn!("SendMonitorLayout callback not set in DispClientContext");
            }
        } else {
            log::error!("DispChannel pointer is NULL, cannot send monitor layout");
        }
    }
}

extern "C" fn disp_caps(
    _context: *mut freerdp_sys::DispClientContext,
    max_num_monitors: freerdp_sys::UINT32,
    max_monitor_area_factor_a: freerdp_sys::UINT32,
    max_monitor_area_factor_b: freerdp_sys::UINT32,
) -> freerdp_sys::UINT32 {
    log::debug!(
        "DISP: Received DisplayControlCaps: {} {} {}",
        max_num_monitors,
        max_monitor_area_factor_a,
        max_monitor_area_factor_b
    );
    0 // CHANNEL_RC_OK
}

/// # Safety
///
/// The context must be valid and the pointer must be valid.
pub unsafe fn register_disp_callbacks(context: *mut freerdp_sys::DispClientContext) {
    unsafe {
        (*context).DisplayControlCaps = Some(disp_caps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disp_channel_new_null() {
        let ch = DispChannel::new(std::ptr::null_mut());
        let debug = format!("{:?}", ch);
        assert!(debug.starts_with("DispChannel"));
    }

    #[test]
    fn test_disp_channel_new_null_send_monitor_layout() {
        // Should not panic with null ptr
        let ch = DispChannel::new(std::ptr::null_mut());
        ch.send_monitor_layout(Rect::new(0, 0, 800, 600), 0, 100, 100);
    }

    #[test]
    fn test_disp_channel_clone() {
        let ch = DispChannel::new(std::ptr::null_mut());
        let cloned = ch.clone();
        let debug = format!("{:?}", cloned);
        assert!(debug.starts_with("DispChannel"));
    }
}
