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
