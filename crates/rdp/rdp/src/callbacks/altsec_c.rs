use freerdp_sys::{
    BOOL, CREATE_NINE_GRID_BITMAP_ORDER, CREATE_OFFSCREEN_BITMAP_ORDER,
    DRAW_GDIPLUS_CACHE_END_ORDER, DRAW_GDIPLUS_CACHE_FIRST_ORDER, DRAW_GDIPLUS_CACHE_NEXT_ORDER,
    DRAW_GDIPLUS_END_ORDER, DRAW_GDIPLUS_FIRST_ORDER, DRAW_GDIPLUS_NEXT_ORDER, FRAME_MARKER_ORDER,
    STREAM_BITMAP_FIRST_ORDER, STREAM_BITMAP_NEXT_ORDER, SWITCH_SURFACE_ORDER, UINT8, rdpContext,
};

use super::{
    super::{connection::context::OwnerFromCtx, utils::ToStringLossy},
    altsec::AltSecCallbacks,
};

use shared::log;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Callbacks {
    CreateOffscreenBitmap,
    SwitchSurface,
    CreateNineGridBitmap,
    FrameMarker,
    StreamBitmapFirst,
    StreamBitmapNext,
    DrawGdiPlusFirst,
    DrawGdiPlusNext,
    DrawGdiPlusEnd,
    DrawGdiPlusCacheFirst,
    DrawGdiPlusCacheNext,
    DrawGdiPlusCacheEnd,
    DrawOrderInfo,
}

impl Callbacks {
    #[allow(dead_code)]
    pub fn all() -> Vec<Callbacks> {
        vec![
            Callbacks::CreateOffscreenBitmap,
            Callbacks::SwitchSurface,
            Callbacks::CreateNineGridBitmap,
            Callbacks::FrameMarker,
            Callbacks::StreamBitmapFirst,
            Callbacks::StreamBitmapNext,
            Callbacks::DrawGdiPlusFirst,
            Callbacks::DrawGdiPlusNext,
            Callbacks::DrawGdiPlusEnd,
            Callbacks::DrawGdiPlusCacheFirst,
            Callbacks::DrawGdiPlusCacheNext,
            Callbacks::DrawGdiPlusCacheEnd,
            Callbacks::DrawOrderInfo,
        ]
    }
}

/// # Safety
///
/// Interoperability with C code.
/// Ensure that the context pointer is valid.
pub unsafe fn set_callbacks(context: *mut rdpContext, overrides: &[Callbacks]) {
    unsafe {
        let update = (*context).update;
        let altsec = (*update).altsec;
        if update.is_null() || altsec.is_null() {
            log::debug!(" 🧪 **** AltSec not initialized, cannot override callbacks.");
            return;
        }
        for override_cb in overrides {
            match override_cb {
                Callbacks::CreateOffscreenBitmap => {
                    (*altsec).CreateOffscreenBitmap = Some(create_offscreen_bitmap);
                }
                Callbacks::SwitchSurface => {
                    (*altsec).SwitchSurface = Some(switch_surface);
                }
                Callbacks::CreateNineGridBitmap => {
                    (*altsec).CreateNineGridBitmap = Some(create_nine_grid_bitmap);
                }
                Callbacks::FrameMarker => {
                    (*altsec).FrameMarker = Some(frame_marker);
                }
                Callbacks::StreamBitmapFirst => {
                    (*altsec).StreamBitmapFirst = Some(stream_bitmap_first);
                }
                Callbacks::StreamBitmapNext => {
                    (*altsec).StreamBitmapNext = Some(stream_bitmap_next);
                }
                Callbacks::DrawGdiPlusFirst => {
                    (*altsec).DrawGdiPlusFirst = Some(draw_gdi_plus_first);
                }
                Callbacks::DrawGdiPlusNext => {
                    (*altsec).DrawGdiPlusNext = Some(draw_gdi_plus_next);
                }
                Callbacks::DrawGdiPlusEnd => {
                    (*altsec).DrawGdiPlusEnd = Some(draw_gdi_plus_end);
                }
                Callbacks::DrawGdiPlusCacheFirst => {
                    (*altsec).DrawGdiPlusCacheFirst = Some(draw_gdi_plus_cache_first);
                }
                Callbacks::DrawGdiPlusCacheNext => {
                    (*altsec).DrawGdiPlusCacheNext = Some(draw_gdi_plus_cache_next);
                }
                Callbacks::DrawGdiPlusCacheEnd => {
                    (*altsec).DrawGdiPlusCacheEnd = Some(draw_gdi_plus_cache_end);
                }
                Callbacks::DrawOrderInfo => {
                    (*altsec).DrawOrderInfo = Some(draw_order_info);
                }
            }
        }
    }
}

extern "C" fn create_offscreen_bitmap(
    context: *mut rdpContext,
    create_offscreen_bitmap: *const CREATE_OFFSCREEN_BITMAP_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner
            .on_create_offscreen_bitmap(create_offscreen_bitmap)
            .into()
    } else {
        true.into()
    }
}

extern "C" fn switch_surface(
    context: *mut rdpContext,
    switch_surface: *const SWITCH_SURFACE_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_switch_surface(switch_surface).into()
    } else {
        true.into()
    }
}

extern "C" fn create_nine_grid_bitmap(
    context: *mut rdpContext,
    create_nine_grid_bitmap: *const CREATE_NINE_GRID_BITMAP_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner
            .on_create_nine_grid_bitmap(create_nine_grid_bitmap)
            .into()
    } else {
        true.into()
    }
}

extern "C" fn frame_marker(
    context: *mut rdpContext,
    frame_marker: *const FRAME_MARKER_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_frame_marker(frame_marker).into()
    } else {
        true.into()
    }
}

extern "C" fn stream_bitmap_first(
    context: *mut rdpContext,
    bitmap_data: *const STREAM_BITMAP_FIRST_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_stream_bitmap_first(bitmap_data).into()
    } else {
        true.into()
    }
}

extern "C" fn stream_bitmap_next(
    context: *mut rdpContext,
    bitmap_data: *const STREAM_BITMAP_NEXT_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_stream_bitmap_next(bitmap_data).into()
    } else {
        true.into()
    }
}

extern "C" fn draw_gdi_plus_first(
    context: *mut rdpContext,
    bitmap_data: *const DRAW_GDIPLUS_FIRST_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_draw_gdi_plus_first(bitmap_data).into()
    } else {
        true.into()
    }
}

extern "C" fn draw_gdi_plus_next(
    context: *mut rdpContext,
    bitmap_data: *const DRAW_GDIPLUS_NEXT_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_draw_gdi_plus_next(bitmap_data).into()
    } else {
        true.into()
    }
}

extern "C" fn draw_gdi_plus_end(
    context: *mut rdpContext,
    bitmap_data: *const DRAW_GDIPLUS_END_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_draw_gdi_plus_end(bitmap_data).into()
    } else {
        true.into()
    }
}

extern "C" fn draw_gdi_plus_cache_first(
    context: *mut rdpContext,
    bitmap_data: *const DRAW_GDIPLUS_CACHE_FIRST_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_draw_gdi_plus_cache_first(bitmap_data).into()
    } else {
        true.into()
    }
}
    
extern "C" fn draw_gdi_plus_cache_next(
    context: *mut rdpContext,
    bitmap_data: *const DRAW_GDIPLUS_CACHE_NEXT_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_draw_gdi_plus_cache_next(bitmap_data).into()
    } else {
        true.into()
    }
}

extern "C" fn draw_gdi_plus_cache_end(
    context: *mut rdpContext,
    bitmap_data: *const DRAW_GDIPLUS_CACHE_END_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_draw_gdi_plus_cache_end(bitmap_data).into()
    } else {
        true.into()
    }
}

extern "C" fn draw_order_info(
    context: *mut rdpContext,
    order_type: UINT8,
    order_name: *const ::std::os::raw::c_char,
) -> BOOL {
    if let Some(owner) = context.owner() {
        let order_name = order_name.to_string_lossy();
        owner.on_draw_order_info(order_type, &order_name).into()
    } else {
        true.into()
    }
}
