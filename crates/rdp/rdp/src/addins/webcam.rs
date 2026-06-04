use std::sync::Arc;

use freerdp_sys::{CHANNEL_RC_OK, IDRDYNVC_ENTRY_POINTS, UINT, IWTSPlugin, IWTSVirtualChannelManager};
use multimedia::webcam::WebcamHandle;
use shared::log;

#[repr(C)]
pub struct WebcamPlugin {
    pub plugin: IWTSPlugin,
    pub webcam: Option<Arc<WebcamHandle>>,
}

pub unsafe extern "C" fn webcam_entry(p_entry_points: *mut IDRDYNVC_ENTRY_POINTS) -> UINT {
    if p_entry_points.is_null() {
        return 1;
    }

    let mut plugin = Box::new(WebcamPlugin {
        plugin: IWTSPlugin {
            Initialize: Some(plugin_initialize),
            Connected: None,
            Disconnected: None,
            Terminated: Some(plugin_terminated),
            Attached: Some(plugin_attached),
            Detached: None,
            ..unsafe { std::mem::zeroed() }
        },
        webcam: None,
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

unsafe extern "C" fn plugin_initialize(
    plugin: *mut IWTSPlugin,
    channel_mgr: *mut IWTSVirtualChannelManager,
) -> UINT {
    if plugin.is_null() {
        return 1;
    }
    let wp = unsafe { &mut *(plugin as *mut WebcamPlugin) };
    log::info!("Webcam plugin: Initialize");

    wp.webcam = Some(Arc::new(WebcamHandle::new()));

    // Create listener for the device enumerator channel.
    // When the server connects, Attached callback fires.
    if !channel_mgr.is_null() {
        unsafe {
            if let Some(create_listener) = (*channel_mgr).CreateListener {
                create_listener(
                    channel_mgr,
                    c"RDCamera_Device_Enumerator".as_ptr(),
                    0, // flags
                    std::ptr::null_mut(), // listener callback — null for now
                    std::ptr::null_mut(), // out listener handle
                );
            }
        }
    }

    CHANNEL_RC_OK
}

unsafe extern "C" fn plugin_attached(plugin: *mut IWTSPlugin) -> UINT {
    if plugin.is_null() {
        return 1;
    }
    let wp = unsafe { &mut *(plugin as *mut WebcamPlugin) };
    log::info!("Webcam plugin: Channel attached — starting capture");

    if let Some(ref webcam) = wp.webcam {
        webcam.start_stream(640, 480, 15);
    }

    CHANNEL_RC_OK
}

unsafe extern "C" fn plugin_terminated(plugin: *mut IWTSPlugin) -> UINT {
    if plugin.is_null() {
        return 1;
    }
    let wp = unsafe { &mut *(plugin as *mut WebcamPlugin) };
    log::info!("Webcam plugin: Terminated");

    if let Some(ref webcam) = wp.webcam {
        webcam.close();
    }
    wp.webcam = None;

    CHANNEL_RC_OK
}
