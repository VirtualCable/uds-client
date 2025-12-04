use freerdp_sys::*;

use shared::log;

use crate::{
    callbacks::{
        altsec, channels, entrypoint, graphics, input, instance, pointer_update, primary,
        secondary, update, window,
    },
    utils::{normalize_invalids, SafePtr},
};

use super::{Rdp, RdpMessage};

impl instance::InstanceCallbacks for Rdp {
    fn on_post_connect(&mut self) -> bool {
        log::debug!(" **** Connected successfully!");
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
                    log::debug!(" **** END PAINT no invalid regions, skipping");
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

impl channels::ChannelsCallbacks for Rdp {
    fn on_channel_connected(
        &mut self,
        _size: usize,
        _sender: &str,
        name: &str,
        p_interface: *mut std::os::raw::c_void,
    ) -> bool {
        // Returns true if we accepted the channel connection, false otherwise.
        if name.as_bytes() == &freerdp_sys::RAIL_SVC_CHANNEL_NAME[..freerdp_sys::RAIL_SVC_CHANNEL_NAME.len()-1] {
            log::debug!("**** RAIL channel connection accepted.");
            // TOOD: handle RAIL channel initialization here
            return true;
        }
        if name.as_bytes() == &freerdp_sys::DISP_DVC_CHANNEL_NAME[..freerdp_sys::DISP_DVC_CHANNEL_NAME.len()-1] {
            log::debug!("**** DISP channel connection accepted.");
            // If p_interface is valid, store it
            self.disp = SafePtr::new(p_interface as *mut DispClientContext);
            if let Some(disp) = &self.disp {
                log::debug!("**** DISP channel context stored: {:?}", disp.as_ptr());
            }
            return true;
        }
        if name.as_bytes() == &freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME[..freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME.len()-1] {
            log::debug!("**** CLIPRDR channel connection accepted.");
            // TOOD: handle CLIPRDR channel initialization here
            return true;
        }

        false // Defaults to false, let freerdp handle it.
    }

    fn on_channel_disconnected(
        &mut self,
        size: usize,
        sender: &str,
        name: &str,
        p_interface: *mut std::os::raw::c_void,
    ) -> bool {
        log::debug!(
            "**** ChannelDisconnected Event: size={}, sender={}, name={}, pInterface={:?}",
            size,
            sender,
            name,
            p_interface
        );
        // Returns true if we accepted the channel disconnection, false otherwise.
        if name.as_bytes() == &freerdp_sys::RAIL_SVC_CHANNEL_NAME[..freerdp_sys::RAIL_SVC_CHANNEL_NAME.len()-1] {
            log::debug!(" **** RAIL channel disconnection accepted.");
            // TOOD: handle RAIL channel cleanup here
            return true;
        }
        if name.as_bytes() == &freerdp_sys::DISP_DVC_CHANNEL_NAME[..freerdp_sys::DISP_DVC_CHANNEL_NAME.len()-1] {
            log::debug!("**** DISP channel disconnection accepted.");
            self.disp = None;
            return true;
        }
        if name.as_bytes() == &freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME[..freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME.len()-1] {
            log::debug!("**** CLIPRDR channel disconnection accepted.");
            // TOOD: handle CLIPRDR channel cleanup here
            return true;
        }

        false // Defaults to false, let freerdp handle it.
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
