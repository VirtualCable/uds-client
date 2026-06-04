use std::sync::Arc;

use freerdp_sys::{
    CHANNEL_RC_OK, IDRDYNVC_ENTRY_POINTS, IWTSListener, IWTSPlugin,
    IWTSVirtualChannelManager, UINT,
};
use multimedia::webcam::{WebcamHandle, WebcamMode};
use shared::log;

mod channel;
mod listener;
mod pdu;

#[repr(C)]
pub struct WebcamPlugin {
    pub plugin: IWTSPlugin,
    pub webcam: Option<Arc<WebcamHandle>>,
    pub(crate) listener: Option<*mut IWTSListener>,
    pub(crate) listener_ctx: Option<*mut listener::ListenerCtx>,
}

// ── Entry point ──────────────────────────────────────────

pub unsafe extern "C" fn webcam_entry(p_entry_points: *mut IDRDYNVC_ENTRY_POINTS) -> UINT {
    if p_entry_points.is_null() {
        return 1;
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

    let webcam = Arc::new(WebcamHandle::with_mode(WebcamMode::MJPEG));
    wp.webcam = Some(webcam.clone());

    let (raw_ctx, listener_handle, error) =
        listener::create_listener(webcam, channel_mgr);

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

    if let Some(ref webcam) = wp.webcam {
        webcam.close();
    }
    wp.webcam = None;
    wp.listener = None;

    if let Some(ctx) = wp.listener_ctx.take() {
        unsafe { let _ = Box::from_raw(ctx); }
    }

    unsafe { let _ = Box::from_raw(plugin as *mut WebcamPlugin); }
    CHANNEL_RC_OK
}
