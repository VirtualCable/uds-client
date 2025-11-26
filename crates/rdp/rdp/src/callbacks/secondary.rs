use freerdp_sys::{
    CACHE_BITMAP_ORDER, CACHE_BITMAP_V2_ORDER, CACHE_BITMAP_V3_ORDER, CACHE_BRUSH_ORDER,
    CACHE_COLOR_TABLE_ORDER, CACHE_GLYPH_ORDER, CACHE_GLYPH_V2_ORDER, INT16, UINT8, UINT16,
};

use shared::log;

pub trait SecondaryCallbacks {
    fn on_cache_bitmap(&self, cache_bitmap_order: *const CACHE_BITMAP_ORDER) -> bool {
        log::debug!(
            "SecondaryCallbacks::on_cache_bitmap: cache_bitmap_order={:?}",
            cache_bitmap_order
        );
        true
    }

    fn on_cache_bitmap_v2(&self, cache_bitmap_v2_order: *mut CACHE_BITMAP_V2_ORDER) -> bool {
        log::debug!(
            "SecondaryCallbacks::on_cache_bitmap_v2: cache_bitmap_v2_order={:?}",
            cache_bitmap_v2_order
        );
        true
    }

    fn on_cache_bitmap_v3(&self, cache_bitmap_v3_order: *mut CACHE_BITMAP_V3_ORDER) -> bool {
        log::debug!(
            "SecondaryCallbacks::on_cache_bitmap_v3: cache_bitmap_v3_order={:?}",
            cache_bitmap_v3_order
        );
        true
    }

    fn on_cache_color_table(&self, cache_color_table: *const CACHE_COLOR_TABLE_ORDER) -> bool {
        log::debug!(
            "SecondaryCallbacks::on_cache_color_table: cache_color_table={:?}",
            cache_color_table
        );
        true
    }

    fn on_cache_glyph(&self, cache_glyph: *const CACHE_GLYPH_ORDER) -> bool {
        log::debug!(
            "SecondaryCallbacks::on_cache_glyph: cache_glyph={:?}",
            cache_glyph
        );
        true
    }

    fn on_cache_glyph_v2(&self, cache_glyph_v2: *const CACHE_GLYPH_V2_ORDER) -> bool {
        log::debug!(
            "SecondaryCallbacks::on_cache_glyph_v2: cache_glyph_v2={:?}",
            cache_glyph_v2
        );
        true
    }

    fn on_cache_brush(&self, cache_brush: *const CACHE_BRUSH_ORDER) -> bool {
        log::debug!(
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
        log::debug!(
            "SecondaryCallbacks::on_cache_order_info: order_length={}, extra_flags={}, order_type={}, order_name={}",
            order_length, extra_flags, order_type, order_name
        );
        true
    }
}
