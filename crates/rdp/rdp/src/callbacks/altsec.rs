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

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use freerdp_sys::{
    CREATE_NINE_GRID_BITMAP_ORDER, CREATE_OFFSCREEN_BITMAP_ORDER, DRAW_GDIPLUS_CACHE_END_ORDER,
    DRAW_GDIPLUS_CACHE_FIRST_ORDER, DRAW_GDIPLUS_CACHE_NEXT_ORDER, DRAW_GDIPLUS_END_ORDER,
    DRAW_GDIPLUS_FIRST_ORDER, DRAW_GDIPLUS_NEXT_ORDER, FRAME_MARKER_ORDER,
    STREAM_BITMAP_FIRST_ORDER, STREAM_BITMAP_NEXT_ORDER, SWITCH_SURFACE_ORDER, UINT8,
};

use shared::log::debug;

pub trait AltSecCallbacks {
    fn on_create_offscreen_bitmap(&self, bitmap: *const CREATE_OFFSCREEN_BITMAP_ORDER) -> bool {
        debug!(
            "AltSecCallbacks::on_create_offscreen_bitmap: bitmap={:?}",
            bitmap
        );
        true
    }

    fn on_switch_surface(&self, switch_surface: *const SWITCH_SURFACE_ORDER) -> bool {
        debug!(
            "AltSecCallbacks::on_switch_surface: switch_surface={:?}",
            switch_surface
        );
        true
    }

    fn on_create_nine_grid_bitmap(
        &self,
        create_nine_grid_bitmap: *const CREATE_NINE_GRID_BITMAP_ORDER,
    ) -> bool {
        debug!(
            "AltSecCallbacks::on_create_nine_grid_bitmap: create_nine_grid_bitmap={:?}",
            create_nine_grid_bitmap
        );
        true
    }

    fn on_frame_marker(&self, frame_marker: *const FRAME_MARKER_ORDER) -> bool {
        debug!(
            "AltSecCallbacks::on_frame_marker: frame_marker={:?}",
            frame_marker
        );
        true
    }

    fn on_stream_bitmap_first(&self, bitmap_data: *const STREAM_BITMAP_FIRST_ORDER) -> bool {
        debug!(
            "AltSecCallbacks::on_stream_bitmap_first: bitmap_data={:?}",
            bitmap_data
        );
        true
    }

    fn on_stream_bitmap_next(&self, bitmap_data: *const STREAM_BITMAP_NEXT_ORDER) -> bool {
        debug!(
            "AltSecCallbacks::on_stream_bitmap_next: bitmap_data={:?}",
            bitmap_data
        );
        true
    }

    fn on_draw_gdi_plus_first(&self, bitmap_data: *const DRAW_GDIPLUS_FIRST_ORDER) -> bool {
        debug!(
            "AltSecCallbacks::on_draw_gdi_plus_first: bitmap_data={:?}",
            bitmap_data
        );
        true
    }

    fn on_draw_gdi_plus_next(&self, bitmap_data: *const DRAW_GDIPLUS_NEXT_ORDER) -> bool {
        debug!(
            "AltSecCallbacks::on_draw_gdi_plus_next: bitmap_data={:?}",
            bitmap_data
        );
        true
    }

    fn on_draw_gdi_plus_end(&self, bitmap_data: *const DRAW_GDIPLUS_END_ORDER) -> bool {
        debug!(
            "AltSecCallbacks::on_draw_gdi_plus_end: bitmap_data={:?}",
            bitmap_data
        );
        true
    }

    fn on_draw_gdi_plus_cache_first(
        &self,
        bitmap_data: *const DRAW_GDIPLUS_CACHE_FIRST_ORDER,
    ) -> bool {
        debug!(
            "AltSecCallbacks::on_draw_gdi_plus_cache_first: bitmap_data={:?}",
            bitmap_data
        );
        true
    }

    fn on_draw_gdi_plus_cache_next(
        &self,
        bitmap_data: *const DRAW_GDIPLUS_CACHE_NEXT_ORDER,
    ) -> bool {
        debug!(
            "AltSecCallbacks::on_draw_gdi_plus_cache_next: bitmap_data={:?}",
            bitmap_data
        );
        true
    }

    fn on_draw_gdi_plus_cache_end(&self, bitmap_data: *const DRAW_GDIPLUS_CACHE_END_ORDER) -> bool {
        debug!(
            "AltSecCallbacks::on_draw_gdi_plus_cache_end: bitmap_data={:?}",
            bitmap_data
        );
        true
    }

    fn on_draw_order_info(&self, order_type: UINT8, order_name: &str) -> bool {
        debug!(
            "AltSecCallbacks::on_draw_order_info: order_type={}, order_name={}",
            order_type, order_name
        );
        true
    }
}
