use freerdp_sys::{
    BITMAP_UPDATE, BOOL, BYTE, FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopHeight, FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopWidth, MONITOR_DEF, PALETTE_UPDATE, PLAY_SOUND_UPDATE, RECTANGLE_16, SURFACE_BITS_COMMAND, UINT16, UINT32, freerdp_settings_get_uint32, gdi_resize, rdpBounds, rdpContext, wStream
};

use super::{super::connection::context::OwnerFromCtx, update::UpdateCallbacks};
use shared::log::debug;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Callbacks {
    BeginPaint,
    EndPaint,
    SetBounds,
    Synchronize,
    DesktopResize,
    BitmapUpdate,
    Palette,
    PlaySound,
    SetKeyboardIndicators,
    SetKeyboardImeStatus,
    RefreshRect,
    SuppressOutput,
    RemoteMonitors,
    SurfaceCommand,
    SurfaceBits,
    SurfaceFrameMarker,
    SurfaceFrameBits,
    SurfaceFrameAcknowledge,
    SaveSessionInfo,
    ServerStatusInfo,
}

impl Callbacks {
    #[allow(dead_code)]
    pub fn all() -> Vec<Callbacks> {
        vec![
            Callbacks::BeginPaint,
            Callbacks::EndPaint,
            Callbacks::SetBounds,
            Callbacks::Synchronize,
            Callbacks::DesktopResize,
            Callbacks::BitmapUpdate,
            Callbacks::Palette,
            Callbacks::PlaySound,
            Callbacks::SetKeyboardIndicators,
            Callbacks::SetKeyboardImeStatus,
            Callbacks::RefreshRect,
            Callbacks::SuppressOutput,
            Callbacks::RemoteMonitors,
            Callbacks::SurfaceCommand,
            Callbacks::SurfaceBits,
            Callbacks::SurfaceFrameMarker,
            Callbacks::SurfaceFrameBits,
            Callbacks::SurfaceFrameAcknowledge,
            Callbacks::SaveSessionInfo,
            Callbacks::ServerStatusInfo,
        ]
    }
}

/// # Safety
/// This function is unsafe because it dereferences raw pointers to set callback functions.
pub unsafe fn set_callbacks(context: *mut rdpContext, overrides: &[Callbacks]) {
    unsafe {
        let update = (*context).update;
        if update.is_null() {
            debug!(" ⁉️ **** Update not initialized, cannot override callbacks.");
            return;
        }
        for override_cb in overrides {
            match override_cb {
                Callbacks::BeginPaint => {
                    (*update).BeginPaint = Some(begin_paint);
                }
                Callbacks::EndPaint => {
                    (*update).EndPaint = Some(end_paint);
                }
                Callbacks::SetBounds => {
                    (*update).SetBounds = Some(set_bounds);
                }
                Callbacks::Synchronize => {
                    (*update).Synchronize = Some(synchronize);
                }
                Callbacks::DesktopResize => {
                    (*update).DesktopResize = Some(desktop_resize);
                }
                Callbacks::BitmapUpdate => {
                    (*update).BitmapUpdate = Some(bitmap_update);
                }
                Callbacks::Palette => {
                    (*update).Palette = Some(palette);
                }
                Callbacks::PlaySound => {
                    (*update).PlaySound = Some(play_sound);
                }
                Callbacks::SetKeyboardIndicators => {
                    (*update).SetKeyboardIndicators = Some(set_keyboard_indicators);
                }
                Callbacks::SetKeyboardImeStatus => {
                    (*update).SetKeyboardImeStatus = Some(set_keyboard_ime_status);
                }
                Callbacks::RefreshRect => {
                    (*update).RefreshRect = Some(refresh_rect);
                }
                Callbacks::SuppressOutput => {
                    (*update).SuppressOutput = Some(suppress_output);
                }
                Callbacks::RemoteMonitors => {
                    (*update).RemoteMonitors = Some(remote_monitors);
                }
                Callbacks::SurfaceCommand => {
                    (*update).SurfaceCommand = Some(surface_command);
                }
                Callbacks::SurfaceBits => {
                    (*update).SurfaceBits = Some(surface_bits);
                }
                Callbacks::SurfaceFrameMarker => {
                    (*update).SurfaceFrameMarker = Some(surface_frame_marker);
                }
                Callbacks::SurfaceFrameBits => {
                    (*update).SurfaceFrameBits = Some(surface_frame_bits);
                }
                Callbacks::SurfaceFrameAcknowledge => {
                    (*update).SurfaceFrameAcknowledge = Some(surface_frame_acknowledge);
                }
                Callbacks::SaveSessionInfo => {
                    (*update).SaveSessionInfo = Some(save_session_info);
                }
                Callbacks::ServerStatusInfo => {
                    (*update).ServerStatusInfo = Some(server_status_info);
                }
            }
        }
    }
}

extern "C" fn begin_paint(context: *mut rdpContext) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_begin_paint().into()
    } else {
        true.into()
    }
}

extern "C" fn end_paint(context: *mut rdpContext) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_end_paint().into()
    } else {
        true.into()
    }
}

extern "C" fn set_bounds(context: *mut rdpContext, bounds: *const rdpBounds) -> BOOL {
    debug!(" **** SET BOUNDS called...");
    if let Some(owner) = context.owner() {
        unsafe { owner.on_set_bounds(bounds).into() }
    } else {
        true.into()
    }
}

extern "C" fn synchronize(context: *mut rdpContext) -> BOOL {
    debug!(" **** SYNCHRONIZE called...");
    if let Some(owner) = context.owner() {
        owner.on_synchronize().into()
    } else {
        true.into()
    }
}

extern "C" fn desktop_resize(context: *mut rdpContext) -> BOOL {
    debug!(" **** DESKTOP RESIZE called... {:?}", context);
    unsafe {
        gdi_resize(
            (*context).gdi,
            freerdp_settings_get_uint32(
                (*context).settings,
                FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopWidth,
            ),
            freerdp_settings_get_uint32(
                (*context).settings,
                FreeRDP_Settings_Keys_UInt32_FreeRDP_DesktopHeight,
            ),
        );
    }

    if let Some(owner) = context.owner() {
        owner.on_desktop_resize().into()
    } else {
        true.into()
    }
}

#[allow(dead_code)]
extern "C" fn bitmap_update(context: *mut rdpContext, bitmap: *const BITMAP_UPDATE) -> BOOL {
    debug!(" **** BITMAP UPDATE called...");
    if let Some(owner) = context.owner() {
        owner.on_bitmap_update(bitmap).into()
    } else {
        true.into()
    }
}

#[allow(dead_code)]
extern "C" fn palette(context: *mut rdpContext, palette: *const PALETTE_UPDATE) -> BOOL {
    debug!(" **** PALETTE UPDATE called...");
    if let Some(owner) = context.owner() {
        owner.on_palette(palette).into()
    } else {
        true.into()
    }
}

// (*update).PlaySound = Some(update_c_callbacks::play_sound);
extern "C" fn play_sound(context: *mut rdpContext, play_sound: *const PLAY_SOUND_UPDATE) -> BOOL {
    debug!(" **** PLAY SOUND called...");
    if let Some(owner) = context.owner() {
        owner.on_play_sound(play_sound).into()
    } else {
        true.into()
    }
}

// (*update).SetKeyboardIndicators = Some(update_c_callbacks::set_keyboard_indicators);
extern "C" fn set_keyboard_indicators(context: *mut rdpContext, led_flags: UINT16) -> BOOL {
    debug!(" **** SET KEYBOARD INDICATORS called...");
    if let Some(owner) = context.owner() {
        owner.on_set_keyboard_indicators(led_flags).into()
    } else {
        true.into()
    }
}

// (*update).SetKeyboardImeStatus = Some(update_c_callbacks::set_keyboard_ime_status);
extern "C" fn set_keyboard_ime_status(
    context: *mut rdpContext,
    ime_id: UINT16,
    ime_state: UINT32,
    ime_conv_mode: UINT32,
) -> BOOL {
    debug!(" **** SET KEYBOARD IME STATUS called...");
    if let Some(owner) = context.owner() {
        owner
            .on_set_keyboard_ime_status(ime_id, ime_state, ime_conv_mode)
            .into()
    } else {
        true.into()
    }
}

#[allow(dead_code)]
extern "C" fn refresh_rect(
    context: *mut rdpContext,
    count: BYTE,
    areas: *const RECTANGLE_16,
) -> BOOL {
    debug!(" **** REFRESH RECT called...");
    if let Some(owner) = context.owner() {
        owner.on_refresh_rect(count, areas).into()
    } else {
        true.into()
    }
}

#[allow(dead_code)]
extern "C" fn suppress_output(
    context: *mut rdpContext,
    allow: BYTE,
    area: *const RECTANGLE_16,
) -> BOOL {
    debug!(" **** SUPPRESS OUTPUT called...");
    if let Some(owner) = context.owner() {
        owner.on_suppress_output(allow, area).into()
    } else {
        true.into()
    }
}

extern "C" fn remote_monitors(
    context: *mut rdpContext,
    count: UINT32,
    monitors: *const MONITOR_DEF,
) -> BOOL {
    debug!(" **** REMOTE MONITORS called...");
    if let Some(owner) = context.owner() {
        owner.on_remote_monitors(count, monitors).into()
    } else {
        true.into()
    }
}

// (*update).SurfaceCommand = Some(update_c_callbacks::surface_command);
extern "C" fn surface_command(context: *mut rdpContext, s: *mut wStream) -> BOOL {
    debug!(" **** SURFACE COMMAND called...");
    if let Some(owner) = context.owner() {
        owner.on_surface_command(s).into()
    } else {
        true.into()
    }
}

#[allow(dead_code)]
extern "C" fn surface_bits(
    context: *mut rdpContext,
    surface_bits: *const freerdp_sys::SURFACE_BITS_COMMAND,
) -> BOOL {
    debug!(" **** SURFACE BITS called...");
    if let Some(owner) = context.owner() {
        owner.on_surface_bits(surface_bits).into()
    } else {
        true.into()
    }
}

#[allow(dead_code)]
extern "C" fn surface_frame_marker(
    context: *mut rdpContext,
    surface_frame_marker: *const freerdp_sys::SURFACE_FRAME_MARKER,
) -> BOOL {
    debug!(" **** SURFACE FRAME MARKER called...");
    if let Some(owner) = context.owner() {
        owner.on_surface_frame_marker(surface_frame_marker).into()
    } else {
        true.into()
    }
}

extern "C" fn surface_frame_bits(
    context: *mut rdpContext,
    cmd: *const SURFACE_BITS_COMMAND,
    first: BOOL,
    last: BOOL,
    frame_id: UINT32,
) -> BOOL {
    debug!(" **** SURFACE FRAME BITS called...");
    if let Some(owner) = context.owner() {
        owner
            .on_surface_frame_bits(cmd, first != 0, last != 0, frame_id)
            .into()
    } else {
        true.into()
    }
}

#[allow(dead_code)]
extern "C" fn surface_frame_acknowledge(context: *mut rdpContext, frame_id: UINT32) -> BOOL {
    debug!(" **** SURFACE FRAME ACKNOWLEDGE called...");
    if let Some(owner) = context.owner() {
        owner.on_surface_frame_acknowledge(frame_id).into()
    } else {
        true.into()
    }
}

#[allow(dead_code)]
extern "C" fn save_session_info(
    context: *mut rdpContext,
    type_: UINT32,
    data: *mut ::std::os::raw::c_void,
) -> BOOL {
    debug!(" **** SAVE SESSION INFO called...");
    if let Some(owner) = context.owner() {
        owner.on_save_session_info(type_, data).into()
    } else {
        true.into()
    }
}

extern "C" fn server_status_info(context: *mut rdpContext, status: UINT32) -> BOOL {
    debug!(" **** SERVER STATUS INFO called...");
    if let Some(owner) = context.owner() {
        owner.on_server_status_info(status).into()
    } else {
        true.into()
    }
}
