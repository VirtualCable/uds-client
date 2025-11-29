use freerdp_sys::{
    ChannelConnectedEventArgs, freerdp_client_OnChannelConnectedEventHandler,
    freerdp_client_OnChannelDisconnectedEventHandler, rdpClientContext,
};

use shared::{log::debug};
use crate::utils::ToStringLossy;

pub extern "C" fn on_channel_connected(
    context: *mut ::std::os::raw::c_void,
    e: *const ChannelConnectedEventArgs,
) {
    let context = context as *mut rdpClientContext;
    let size = unsafe { (*e).e.Size as usize };
    let sender = unsafe { (*e).e.Sender }.to_string_lossy();
    let name = unsafe { (*e).name }.to_string_lossy();
    let p_interface = unsafe { (*e).pInterface };

    debug!(
        " ☁️ **** ChannelConnected Event: size={}, sender={}, name={}, pInterface={:?} (context={:?})",
        size, sender, name, p_interface, context
    );

    unsafe {
        freerdp_client_OnChannelConnectedEventHandler(context as *mut _, e);
    }
}

pub extern "C" fn on_channel_disconnected(
    context: *mut ::std::os::raw::c_void,
    e: *const freerdp_sys::ChannelDisconnectedEventArgs,
) {
    let context: *mut freerdp_sys::rdp_client_context = context as *mut rdpClientContext;
    let size = unsafe { (*e).e.Size as usize };
    let sender = unsafe { (*e).e.Sender }.to_string_lossy();
    let name = unsafe { (*e).name }.to_string_lossy();
    let p_interface = unsafe { (*e).pInterface };

    debug!(
        " ☁️ **** ChannelDisconnected Event: size={}, sender={}, name={}, pInterface={:?} (context={:?})",
        size, sender, name, p_interface, context
    );

    unsafe {
        freerdp_client_OnChannelDisconnectedEventHandler(context as *mut _, e);
    }
}
