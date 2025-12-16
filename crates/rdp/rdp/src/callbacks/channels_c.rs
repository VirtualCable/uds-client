// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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

// Note that channels callbacks must be always implemented, we cannot disable them.

use freerdp_sys::{
    ChannelConnectedEventArgs, freerdp_client_OnChannelConnectedEventHandler,
    freerdp_client_OnChannelDisconnectedEventHandler, rdpContext,
};

use shared::log::debug;

use super::{super::context::OwnerFromCtx, channels::ChannelsCallbacks};
use crate::utils::ToStringLossy;

/// # Safety
/// This function is called from the FreeRDP C library when a channel is connected.
pub unsafe extern "C" fn on_channel_connected(
    context: *mut ::std::os::raw::c_void,
    e: *const ChannelConnectedEventArgs,
) {
    let context = context as *mut rdpContext;
    let size = unsafe { (*e).e.Size as usize };
    let sender = unsafe { (*e).e.Sender }.to_string_lossy();
    let name = unsafe { (*e).name }.to_string_lossy();
    let p_interface = unsafe { (*e).pInterface };

    debug!(
        "**** ChannelConnected Event: size={}, sender={}, name={}, pInterface={:?} (context={:?})",
        size, sender, name, p_interface, context
    );

    // Here we get for example the DISP_DVC_CHANNEL_NAME when the display virtual channel is connected.
    if let Some(rdp) = context.owner() {
        // Here we could notify the Rdp instance if needed.
        if rdp.on_channel_connected(size, &sender, &name, p_interface) {
            debug!("++++  {name} Channel connection accepted by Rdp instance.");
            return;
        } else {
            debug!("----  {name} Channel connection not processed by Rdp instance.");
        }
    }

    unsafe {
        freerdp_client_OnChannelConnectedEventHandler(context as *mut _, e);
    }
}

/// # Safety
/// This function is called from the FreeRDP C library when a channel is disconnected.
pub unsafe extern "C" fn on_channel_disconnected(
    context: *mut ::std::os::raw::c_void,
    e: *const freerdp_sys::ChannelDisconnectedEventArgs,
) {
    let context: *mut freerdp_sys::rdpContext = context as *mut rdpContext;
    let size = unsafe { (*e).e.Size as usize };
    let sender = unsafe { (*e).e.Sender }.to_string_lossy();
    let name = unsafe { (*e).name }.to_string_lossy();
    let p_interface = unsafe { (*e).pInterface };

    debug!(
        "**** ChannelDisconnected Event: size={}, sender={}, name={}, pInterface={:?} (context={:?})",
        size, sender, name, p_interface, context
    );

    if let Some(rdp) = context.owner() {
        // Here we could notify the Rdp instance if needed.
        if rdp.on_channel_disconnected(size, &sender, &name, p_interface) {
            debug!("**** Channel disconnection accepted by Rdp instance.");
            return;
        } else {
            debug!("**** Channel disconnection not accepted by Rdp instance.");
        }
    }

    unsafe {
        freerdp_client_OnChannelDisconnectedEventHandler(context as *mut _, e);
    }
}
