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
    CACHE_BITMAP_ORDER, CACHE_BITMAP_V2_ORDER, CACHE_BITMAP_V3_ORDER, CACHE_BRUSH_ORDER,
    CACHE_COLOR_TABLE_ORDER, CACHE_GLYPH_ORDER, CACHE_GLYPH_V2_ORDER, INT16, UINT8, UINT16,
};

use shared::log::debug;

pub trait SecondaryCallbacks {
    fn on_cache_bitmap(&self, cache_bitmap_order: *const CACHE_BITMAP_ORDER) -> bool {
        debug!(
            "SecondaryCallbacks::on_cache_bitmap: cache_bitmap_order={:?}",
            cache_bitmap_order
        );
        true
    }

    fn on_cache_bitmap_v2(&self, cache_bitmap_v2_order: *mut CACHE_BITMAP_V2_ORDER) -> bool {
        debug!(
            "SecondaryCallbacks::on_cache_bitmap_v2: cache_bitmap_v2_order={:?}",
            cache_bitmap_v2_order
        );
        true
    }

    fn on_cache_bitmap_v3(&self, cache_bitmap_v3_order: *mut CACHE_BITMAP_V3_ORDER) -> bool {
        debug!(
            "SecondaryCallbacks::on_cache_bitmap_v3: cache_bitmap_v3_order={:?}",
            cache_bitmap_v3_order
        );
        true
    }

    fn on_cache_color_table(&self, cache_color_table: *const CACHE_COLOR_TABLE_ORDER) -> bool {
        debug!(
            "SecondaryCallbacks::on_cache_color_table: cache_color_table={:?}",
            cache_color_table
        );
        true
    }

    fn on_cache_glyph(&self, cache_glyph: *const CACHE_GLYPH_ORDER) -> bool {
        debug!(
            "SecondaryCallbacks::on_cache_glyph: cache_glyph={:?}",
            cache_glyph
        );
        true
    }

    fn on_cache_glyph_v2(&self, cache_glyph_v2: *const CACHE_GLYPH_V2_ORDER) -> bool {
        debug!(
            "SecondaryCallbacks::on_cache_glyph_v2: cache_glyph_v2={:?}",
            cache_glyph_v2
        );
        true
    }

    fn on_cache_brush(&self, cache_brush: *const CACHE_BRUSH_ORDER) -> bool {
        debug!(
            "SecondaryCallbacks::on_cache_brush: cache_brush={:?}",
            cache_brush
        );
        true
    }

    fn on_cache_order_info(
        &self,
        order_length: INT16,
        extra_flags: UINT16,
        order_type: UINT8,
        order_name: &str,
    ) -> bool {
        debug!(
            "SecondaryCallbacks::on_cache_order_info: order_length={}, extra_flags={}, order_type={}, order_name={}",
            order_length, extra_flags, order_type, order_name
        );
        true
    }
}
