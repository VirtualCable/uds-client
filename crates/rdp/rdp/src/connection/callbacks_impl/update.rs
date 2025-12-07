use freerdp_sys::*;

use shared::log;

use crate::{callbacks::update, utils::nomralize_rects};

use super::{Rdp, RdpMessage};

impl update::UpdateCallbacks for Rdp {
    fn on_begin_paint(&mut self) -> bool {
        // Note: Regions are cleared by update_begin_paint by FreeRDP itself
        // Left this code here for rerefence, until we are sure it's not needed :).

        // let gdi = self.gdi();
        // unsafe {
        //     let primary = &mut *(*gdi).primary;
        //     let hwnd = (*primary.hdc).hwnd;

        //     if hwnd.is_null() {
        //         return true;
        //     }

        //     if (*hwnd).invalid.is_null() {
        //         return true;
        //     }

        //     // Reset invalid region
        //     (*(*hwnd).invalid).null = true.into();
        //     (*hwnd).ninvalid = 0;
        // }

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
                    if (*gdi).suppressOutput != 0
                        || (*hwnd).invalid.is_null()
                        || (*(*hwnd).invalid).null != 0
                    {
                        return true;
                    }

                    let ninvalid = (*hwnd).ninvalid;
                    let cinvalid = (*hwnd).cinvalid;

                    log::debug!("Rects count: {}", ninvalid);

                    if ninvalid <= 0 {
                        // No invalid regions, skip
                        return true;
                    }
                    (
                        std::slice::from_raw_parts(cinvalid, ninvalid as usize),
                        width,
                        height,
                    )
                };

                if let Some(rects) = nomralize_rects(rects_raw, width, height) {
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
