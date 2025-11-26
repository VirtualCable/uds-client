use freerdp_sys::{
    BOOL, CACHE_BITMAP_ORDER, CACHE_BITMAP_V2_ORDER, CACHE_BITMAP_V3_ORDER, CACHE_BRUSH_ORDER,
    CACHE_COLOR_TABLE_ORDER, CACHE_GLYPH_ORDER, CACHE_GLYPH_V2_ORDER, INT16, UINT8, UINT16,
    rdpContext,
};

use super::{
    super::connection::context::OwnerFromCtx, super::utils::ToStringLossy,
    secondary::SecondaryCallbacks,
};

use shared::log;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Callbacks {
    CacheBitmap,
    CacheBitmapV2,
    CacheBitmapV3,
    CacheColorTable,
    CacheGlyph,
    CacheGlyphV2,
    CacheBrush,
    CacheOrderInfo,
}

impl Callbacks {
    #[allow(dead_code)]
    pub fn all() -> Vec<Callbacks> {
        vec![
            Callbacks::CacheBitmap,
            Callbacks::CacheBitmapV2,
            Callbacks::CacheBitmapV3,
            Callbacks::CacheColorTable,
            Callbacks::CacheGlyph,
            Callbacks::CacheGlyphV2,
            Callbacks::CacheBrush,
            Callbacks::CacheOrderInfo,
        ]
    }
}

/// # Safety
/// Interoperability with C code.
/// Ensure that the context pointer is valid.
pub unsafe fn set_callbacks(context: *mut rdpContext, overrides: &[Callbacks]) {
    unsafe {
        let update = (*context).update;
        let secondary = (*update).secondary;
        if update.is_null() || secondary.is_null() {
            log::debug!(" 🧪 **** Secondary not initialized, cannot override callbacks.");
            return;
        }
        for override_cb in overrides {
            match override_cb {
                Callbacks::CacheBitmap => {
                    (*secondary).CacheBitmap = Some(cache_bitmap);
                }
                Callbacks::CacheBitmapV2 => {
                    (*secondary).CacheBitmapV2 = Some(cache_bitmap_v2);
                }
                Callbacks::CacheBitmapV3 => {
                    (*secondary).CacheBitmapV3 = Some(cache_bitmap_v3);
                }
                Callbacks::CacheColorTable => {
                    (*secondary).CacheColorTable = Some(cache_color_table);
                }
                Callbacks::CacheGlyph => {
                    (*secondary).CacheGlyph = Some(cache_glyph);
                }
                Callbacks::CacheGlyphV2 => {
                    (*secondary).CacheGlyphV2 = Some(cache_glyph_v2);
                }
                Callbacks::CacheBrush => {
                    (*secondary).CacheBrush = Some(cache_brush);
                }
                Callbacks::CacheOrderInfo => {
                    (*secondary).CacheOrderInfo = Some(cache_order_info);
                }
            }
        }
    }
}

pub extern "C" fn cache_bitmap(
    context: *mut rdpContext,
    cache_bitmap_order: *const CACHE_BITMAP_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_cache_bitmap(cache_bitmap_order).into()
    } else {
        true.into()
    }
}

pub extern "C" fn cache_bitmap_v2(
    context: *mut rdpContext,
    cache_bitmap_v2_order: *mut CACHE_BITMAP_V2_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_cache_bitmap_v2(cache_bitmap_v2_order).into()
    } else {
        true.into()
    }
}

pub extern "C" fn cache_bitmap_v3(
    context: *mut rdpContext,
    cache_bitmap_v3_order: *mut CACHE_BITMAP_V3_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_cache_bitmap_v3(cache_bitmap_v3_order).into()
    } else {
        true.into()
    }
}

pub extern "C" fn cache_color_table(
    context: *mut rdpContext,
    cache_color_table: *const CACHE_COLOR_TABLE_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_cache_color_table(cache_color_table).into()
    } else {
        true.into()
    }
}

pub extern "C" fn cache_glyph(
    context: *mut rdpContext,
    cache_glyph: *const CACHE_GLYPH_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_cache_glyph(cache_glyph).into()
    } else {
        true.into()
    }
}

pub extern "C" fn cache_glyph_v2(
    context: *mut rdpContext,
    cache_glyph_v2: *const CACHE_GLYPH_V2_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_cache_glyph_v2(cache_glyph_v2).into()
    } else {
        true.into()
    }
}

pub extern "C" fn cache_brush(
    context: *mut rdpContext,
    cache_brush: *const CACHE_BRUSH_ORDER,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_cache_brush(cache_brush).into()
    } else {
        true.into()
    }
}

pub extern "C" fn cache_order_info(
    context: *mut rdpContext,
    order_length: INT16,
    extra_flags: UINT16,
    order_type: UINT8,
    order_name: *const ::std::os::raw::c_char,
) -> BOOL {
    if let Some(owner) = context.owner() {
        let order_name = order_name.to_string_lossy();
        owner
            .on_cache_order_info(order_length, extra_flags, order_type, &order_name)
            .into()
    } else {
        true.into()
    }
}
