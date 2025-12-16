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
use freerdp_sys::{
    BOOL, CACHE_BITMAP_ORDER, CACHE_BITMAP_V2_ORDER, CACHE_BITMAP_V3_ORDER, CACHE_BRUSH_ORDER,
    CACHE_COLOR_TABLE_ORDER, CACHE_GLYPH_ORDER, CACHE_GLYPH_V2_ORDER, INT16, UINT8, UINT16,
    rdpContext,
};

use super::{
    super::utils::{ToStringLossy},
    super::context::OwnerFromCtx,
    secondary::SecondaryCallbacks,
};
use shared::log::debug;

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
/// This function is unsafe because it dereferences raw pointers to set callback functions.
pub unsafe fn set_callbacks(context: *mut rdpContext, overrides: &[Callbacks]) {
    unsafe {
        let update = (*context).update;
        let secondary = (*update).secondary;
        if update.is_null() || secondary.is_null() {
            debug!(" **** Secondary not initialized, cannot override callbacks.");
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
