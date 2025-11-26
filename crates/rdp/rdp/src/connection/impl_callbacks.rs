use freerdp_sys::*;

use shared::log;

use crate::{
    callbacks::{
        altsec, entrypoint, graphics, input, instance, pointer_update, primary, secondary, update,
        window,
    },
    utils::normalize_invalids,
};

use super::{Rdp, RdpMessage};

impl instance::InstanceCallbacks for Rdp {
    fn on_post_connect(&mut self) -> bool {
        log::debug!(" 🧪 **** Connected successfully!");
        true
    }
}

impl update::UpdateCallbacks for Rdp {
    fn on_begin_paint(&mut self) -> bool {
        let gdi = self.context().unwrap().context().gdi;
        let primary = unsafe { &mut *(*gdi).primary };
        let hwnd = unsafe { (*primary.hdc).hwnd };

        // Reset invalid region
        unsafe { (*(*hwnd).invalid).null = true.into() };
        unsafe { (*hwnd).ninvalid = 0 };

        true
    }

    fn on_end_paint(&mut self) -> bool {
        // If no sender, skip
        if let Some(tx) = &self.update_tx {
            // If no updates, skip
            if let Some(gdi) = self.gdi() {
                let primary = unsafe { &mut *(*gdi).primary };
                let width = unsafe { (*gdi).width } as u32;
                let height = unsafe { (*gdi).height } as u32;
                let hwnd = unsafe { (*primary.hdc).hwnd };

                let ninvalid = unsafe { (*hwnd).ninvalid };
                let cinvalid = unsafe { (*hwnd).invalid };
                if ninvalid <= 0 {
                    log::debug!(" 🖥️ **** END PAINT no invalid regions, skipping");
                    return true;
                }
                let rects_raw = unsafe { std::slice::from_raw_parts(cinvalid, ninvalid as usize) };

                if let Some(rects) = normalize_invalids(rects_raw, width, height) {
                    let _ = tx.try_send(RdpMessage::UpdateRects(rects));
                }
            }
        }
        true
    }

    fn on_desktop_resize(&mut self) -> bool {
        log::debug!(" 🧪 **** Desktop resized");
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
impl input::InputCallbacks for Rdp {}
impl pointer_update::PointerCallbacks for Rdp {}
impl primary::PrimaryCallbacks for Rdp {}
impl secondary::SecondaryCallbacks for Rdp {}
impl altsec::AltSecCallbacks for Rdp {}
impl window::WindowCallbacks for Rdp {}
impl entrypoint::EntrypointCallbacks for Rdp {}
impl graphics::GraphicsCallbacks for Rdp {}
