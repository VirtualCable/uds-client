// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
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

use std::sync::Arc;

use crate::context::OwnerFromCtx;
use crate::integrations::WebcamIntegration;
use crate::utils::log;
use freerdp_sys::{
    CHANNEL_RC_OK, IDRDYNVC_ENTRY_POINTS, IWTSListener, IWTSPlugin, IWTSVirtualChannelManager, UINT,
};

mod channel;
mod listener;
mod pdu;

#[repr(C)]
pub struct WebcamPlugin {
    pub plugin: IWTSPlugin,
    pub webcam: Option<Arc<dyn WebcamIntegration>>,
    pub(crate) rdpcontext: *mut freerdp_sys::rdpContext,
    pub(crate) listener: Option<*mut IWTSListener>,
    pub(crate) listener_ctx: Option<*mut listener::ControlListenerCtx>,
}

// ── Entry point ──────────────────────────────────────────

pub unsafe extern "C" fn webcam_entry(p_entry_points: *mut IDRDYNVC_ENTRY_POINTS) -> UINT {
    if p_entry_points.is_null() {
        return 1;
    }

    let mut rdpcontext: *mut freerdp_sys::rdpContext = std::ptr::null_mut();
    unsafe {
        if let Some(get_ctx) = (*p_entry_points).GetRdpContext {
            rdpcontext = get_ctx(p_entry_points);
        }
    }

    let mut plugin = Box::new(WebcamPlugin {
        plugin: IWTSPlugin {
            Initialize: Some(initialize),
            Connected: None,
            Disconnected: None,
            Terminated: Some(terminated),
            Attached: Some(attached),
            Detached: None,
            ..unsafe { std::mem::zeroed() }
        },
        webcam: None,
        rdpcontext,
        listener: None,
        listener_ctx: None,
    });

    let error = unsafe {
        (*p_entry_points).RegisterPlugin.unwrap_unchecked()(
            p_entry_points,
            c"rdpecam".as_ptr(),
            &mut plugin.plugin,
        )
    };

    if error != CHANNEL_RC_OK {
        log::error!("Webcam plugin registration failed: {error}");
        return error;
    }

    let _ = Box::into_raw(plugin);
    CHANNEL_RC_OK
}

// ── IWTSPlugin callbacks ─────────────────────────────────

unsafe extern "C" fn initialize(
    plugin: *mut IWTSPlugin,
    channel_mgr: *mut IWTSVirtualChannelManager,
) -> UINT {
    if plugin.is_null() || channel_mgr.is_null() {
        return 1;
    }
    let wp = unsafe { &mut *(plugin as *mut WebcamPlugin) };
    log::info!("Webcam plugin: Initialize");

    let rdp = if !wp.rdpcontext.is_null() {
        wp.rdpcontext.owner()
    } else {
        None
    };

    let webcam = if let Some(ref rdp_obj) = rdp {
        if let Some(ref webcam_integration) = rdp_obj.config.integrations.webcam {
            webcam_integration.clone()
        } else {
            log::warn!("Webcam integration not configured");
            return 1;
        }
    } else {
        log::error!("Failed to obtain Rdp owner context in webcam initialize");
        return 1;
    };

    wp.webcam = Some(webcam.clone());

    let (raw_ctx, listener_handle, error) = listener::create_listener(webcam, channel_mgr);

    if error != CHANNEL_RC_OK {
        log::error!("Webcam: CreateListener failed with {error}");
        return error;
    }

    wp.listener_ctx = Some(raw_ctx);
    wp.listener = Some(listener_handle);
    log::info!("Webcam plugin: Listener created");

    CHANNEL_RC_OK
}

unsafe extern "C" fn attached(_plugin: *mut IWTSPlugin) -> UINT {
    log::info!("Webcam plugin: Attached");
    CHANNEL_RC_OK
}

unsafe extern "C" fn terminated(plugin: *mut IWTSPlugin) -> UINT {
    if plugin.is_null() {
        return 1;
    }
    let wp = unsafe { &mut *(plugin as *mut WebcamPlugin) };
    log::info!("Webcam plugin: Terminated");
    unsafe {
        if let Some(l) = wp.listener.take() {
            // we don't have direct access to channel manager here, but we can clean up
            log::debug!("Webcam: Terminated called, listener was: {:?}", l);
        }
        if let Some(ctx) = wp.listener_ctx.take() {
            let _ = Box::from_raw(ctx);
        }
    }
    CHANNEL_RC_OK
}
