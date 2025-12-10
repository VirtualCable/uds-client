use freerdp_sys::*;

use shared::log;

use crate::{callbacks::channels, channels::cliprdr::register_cliprdr_callbacks};

use super::Rdp;

impl channels::ChannelsCallbacks for Rdp {
    fn on_channel_connected(
        &mut self,
        _size: usize,
        _sender: &str,
        name: &str,
        p_interface: *mut std::os::raw::c_void,
    ) -> bool {
        match name.as_bytes() {
            name if name
                == &freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME
                    [..freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME.len() - 1] =>
            {
                if !self.config.settings.clipboard_redirection {
                    log::debug!("**** CLIPRDR channel connection rejected (disabled in settings).");
                    return false;
                }
                let interface = p_interface as *mut CliprdrClientContext;
                unsafe {
                    (*interface).custom =
                        self.context().unwrap() as *const _ as *mut std::os::raw::c_void;
                    register_cliprdr_callbacks(&mut *interface);
                }

                log::debug!("**** CLIPRDR channel connection accepted.");
                self.channels.write().unwrap().set_cliprdr_ptr(interface);
                true
            }
            name if name
                == &freerdp_sys::RAIL_SVC_CHANNEL_NAME
                    [..freerdp_sys::RAIL_SVC_CHANNEL_NAME.len() - 1] =>
            {
                log::debug!("**** RAIL channel connection accepted.");
                true
            }
            name if name
                == &freerdp_sys::DISP_DVC_CHANNEL_NAME
                    [..freerdp_sys::DISP_DVC_CHANNEL_NAME.len() - 1] =>
            {
                log::debug!("**** DISP channel connection accepted.");
                self.channels
                    .write()
                    .unwrap()
                    .set_disp_ptr(p_interface as *mut DispClientContext);
                true
            }
            _ => false, // Defaults to false
        }
    }

    fn on_channel_disconnected(
        &mut self,
        _size: usize,
        _sender: &str,
        name: &str,
        _p_interface: *mut std::os::raw::c_void,
    ) -> bool {
        match name.as_bytes() {
            name if name
                == &freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME
                    [..freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME.len() - 1] =>
            {
                log::debug!("**** CLIPRDR channel disconnected.");
                self.channels.write().unwrap().clear_cliprdr();
                true
            }
            name if name
                == &freerdp_sys::RAIL_SVC_CHANNEL_NAME
                    [..freerdp_sys::RAIL_SVC_CHANNEL_NAME.len() - 1] =>
            {
                log::debug!("**** RAIL channel disconnected.");
                true
            }
            name if name
                == &freerdp_sys::DISP_DVC_CHANNEL_NAME
                    [..freerdp_sys::DISP_DVC_CHANNEL_NAME.len() - 1] =>
            {
                log::debug!("**** DISP channel disconnected.");
                self.channels.write().unwrap().clear_disp();
                true
            }
            _ => false, // Defaults to false
        }
    }
}
