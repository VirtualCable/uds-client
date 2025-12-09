use crate::utils;

use crate::geom::Rect;
use shared::log;

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
        log::debug!("Sending monitor layout: {:?}", rect);
        if let Some(ptr) = &self.ptr {
            // We need the disp channel to send the resize request, not alredy implemented in our code
            // Note: avoid too fast resizing, as it may cause issues
            // with the server or client. (simply, implement a delay or debounce mechanism os 200ms or so)
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
                let _ = unsafe {
                    func(
                        ptr.as_mut_ptr(),
                        dcml_vec.len() as freerdp_sys::UINT32,
                        dcml_vec.as_mut_ptr(),
                    )
                };
            } else {
                log::debug!("SendMonitorLayout callback not set");
            }
        }
    }
}
