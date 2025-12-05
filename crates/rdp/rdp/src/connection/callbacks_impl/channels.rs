use freerdp_sys::*;

use shared::log;

use crate::{callbacks::channels, utils::SafePtr};

use super::Rdp;

impl channels::ChannelsCallbacks for Rdp {
    fn on_channel_connected(
        &mut self,
        _size: usize,
        _sender: &str,
        name: &str,
        p_interface: *mut std::os::raw::c_void,
    ) -> bool {
        // Returns true if we accepted the channel connection, false otherwise.
        if name.as_bytes()
            == &freerdp_sys::RAIL_SVC_CHANNEL_NAME[..freerdp_sys::RAIL_SVC_CHANNEL_NAME.len() - 1]
        {
            log::debug!("**** RAIL channel connection accepted.");
            // TOOD: handle RAIL channel initialization here
            return true;
        }
        if name.as_bytes()
            == &freerdp_sys::DISP_DVC_CHANNEL_NAME[..freerdp_sys::DISP_DVC_CHANNEL_NAME.len() - 1]
        {
            log::debug!("**** DISP channel connection accepted.");
            // If p_interface is valid, store it
            self.disp = SafePtr::new(p_interface as *mut DispClientContext);
            if let Some(disp) = &self.disp {
                log::debug!("**** DISP channel context stored: {:?}", disp.as_ptr());
            }
            return true;
        }
        if name.as_bytes()
            == &freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME
                [..freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME.len() - 1]
        {
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
        if name.as_bytes()
            == &freerdp_sys::RAIL_SVC_CHANNEL_NAME[..freerdp_sys::RAIL_SVC_CHANNEL_NAME.len() - 1]
        {
            log::debug!(" **** RAIL channel disconnection accepted.");
            // TOOD: handle RAIL channel cleanup here
            return true;
        }
        if name.as_bytes()
            == &freerdp_sys::DISP_DVC_CHANNEL_NAME[..freerdp_sys::DISP_DVC_CHANNEL_NAME.len() - 1]
        {
            log::debug!("**** DISP channel disconnection accepted.");
            self.disp = None;
            return true;
        }
        if name.as_bytes()
            == &freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME
                [..freerdp_sys::CLIPRDR_SVC_CHANNEL_NAME.len() - 1]
        {
            log::debug!("**** CLIPRDR channel disconnection accepted.");
            // TOOD: handle CLIPRDR channel cleanup here
            return true;
        }

        false // Defaults to false, let freerdp handle it.
    }
}
