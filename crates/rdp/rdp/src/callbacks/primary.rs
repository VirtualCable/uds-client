use shared::log::debug;
use freerdp_sys::{
    DRAW_NINE_GRID_ORDER, DSTBLT_ORDER, ELLIPSE_CB_ORDER, ELLIPSE_SC_ORDER, FAST_GLYPH_ORDER,
    FAST_INDEX_ORDER, GLYPH_INDEX_ORDER, LINE_TO_ORDER, MEM3BLT_ORDER, MEMBLT_ORDER,
    MULTI_DRAW_NINE_GRID_ORDER, MULTI_DSTBLT_ORDER, MULTI_OPAQUE_RECT_ORDER, MULTI_PATBLT_ORDER,
    MULTI_SCRBLT_ORDER, OPAQUE_RECT_ORDER, ORDER_INFO, PATBLT_ORDER, POLYGON_CB_ORDER,
    POLYGON_SC_ORDER, POLYLINE_ORDER, SAVE_BITMAP_ORDER, SCRBLT_ORDER,
};

pub trait PrimaryCallbacks {
    fn on_dst_blt(&self, dstblt: *const DSTBLT_ORDER) -> bool {
        debug!(" 🧪 **** Default on_dst_blt called: dstblt={:?}", dstblt);
        true
    }

    fn on_pat_blt(&self, patblt: *mut PATBLT_ORDER) -> bool {
        debug!(" 🧪 **** Default on_pat_blt called: patblt={:?}", patblt);
        true
    }

    fn on_scr_blt(&self, scrblt: *const SCRBLT_ORDER) -> bool {
        debug!(" 🧪 **** Default on_scr_blt called: scrblt={:?}", scrblt);
        true
    }

    fn on_opaque_rect(&self, opaque_rect: *const OPAQUE_RECT_ORDER) -> bool {
        debug!(
            " 🧪 **** Default on_opaque_rect called: opaque_rect={:?}",
            opaque_rect
        );
        true
    }

    fn on_draw_nine_grid(&self, draw_nine_grid: *const DRAW_NINE_GRID_ORDER) -> bool {
        debug!(
            " 🧪 **** Default on_draw_nine_grid called: draw_nine_grid={:?}",
            draw_nine_grid
        );
        true
    }

    fn on_multi_dst_blt(&self, multi_dstblt: *const MULTI_DSTBLT_ORDER) -> bool {
        debug!(
            " 🧪 **** Default on_multi_dst_blt called: multi_dstblt={:?}",
            multi_dstblt
        );
        true
    }

    fn on_multi_pat_blt(&self, multi_patblt: *const MULTI_PATBLT_ORDER) -> bool {
        debug!(
            " 🧪 **** Default on_multi_pat_blt called: multi_patblt={:?}",
            multi_patblt
        );
        true
    }

    fn on_multi_scr_blt(&self, multi_scrblt: *const MULTI_SCRBLT_ORDER) -> bool {
        debug!(
            " 🧪 **** Default on_multi_scr_blt called: multi_scrblt={:?}",
            multi_scrblt
        );
        true
    }

    fn on_multi_opaque_rect(&self, multi_opaque_rect: *const MULTI_OPAQUE_RECT_ORDER) -> bool {
        debug!(
            " 🧪 **** Default on_multi_opaque_rect called: multi_opaque_rect={:?}",
            multi_opaque_rect
        );
        true
    }

    fn on_multi_draw_nine_grid(
        &self,
        multi_draw_nine_grid: *const MULTI_DRAW_NINE_GRID_ORDER,
    ) -> bool {
        debug!(
            " 🧪 **** Default on_multi_draw_nine_grid called: multi_draw_nine_grid={:?}",
            multi_draw_nine_grid
        );
        true
    }

    fn on_line_to(&self, line_to: *const LINE_TO_ORDER) -> bool {
        debug!(" 🧪 **** Default on_line_to called: line_to={:?}", line_to);
        true
    }

    fn on_polyline(&self, polyline: *const POLYLINE_ORDER) -> bool {
        debug!(" 🧪 **** Default on_polyline called: polyline={:?}", polyline);
        true
    }

    fn on_mem_blt(&self, memblt: *mut MEMBLT_ORDER) -> bool {
        debug!(" 🧪 **** Default on_mem_blt called: memblt={:?}", memblt);
        true
    }

    fn on_mem3_blt(&self, mem3blt: *mut MEM3BLT_ORDER) -> bool {
        debug!(" 🧪 **** Default on_mem3_blt called: mem3blt={:?}", mem3blt);
        true
    }

    fn on_save_bitmap(&self, bitmap_data: *const SAVE_BITMAP_ORDER) -> bool {
        debug!(
            " 🧪 **** Default on_save_bitmap called: bitmap_data={:?}",
            bitmap_data
        );
        true
    }

    fn on_glyph_index(&self, glyph: *mut GLYPH_INDEX_ORDER) -> bool {
        debug!(" 🧪 **** Default on_glyph_index called: glyph={:?}", glyph);
        true
    }

    fn on_fast_index(&self, glyph: *const FAST_INDEX_ORDER) -> bool {
        debug!(" 🧪 **** Default on_fast_index called: glyph={:?}", glyph);
        true
    }

    fn on_fast_glyph(&self, glyph: *const FAST_GLYPH_ORDER) -> bool {
        debug!(" 🧪 **** Default on_fast_glyph called: glyph={:?}", glyph);
        true
    }

    fn on_polygon_sc(&self, polygon_sc: *const POLYGON_SC_ORDER) -> bool {
        debug!(" 🧪 **** Default on_polygon_sc called: polygon_sc={:?}", polygon_sc);
        true
    }

    fn on_polygon_cb(&self, polygon_cb: *mut POLYGON_CB_ORDER) -> bool {
        debug!(" 🧪 **** Default on_polygon_cb called: polygon_cb={:?}", polygon_cb);
        true
    }

    fn on_ellipse_sc(&self, ellipse_sc: *const ELLIPSE_SC_ORDER) -> bool {
        debug!(" 🧪 **** Default on_ellipse_sc called: ellipse_sc={:?}", ellipse_sc);
        true
    }

    fn on_ellipse_cb(&self, ellipse_cb: *const ELLIPSE_CB_ORDER) -> bool {
        debug!(" 🧪 **** Default on_ellipse_cb called: ellipse_cb={:?}", ellipse_cb);
        true
    }

    fn on_order_info(&self, order_info: *const ORDER_INFO, order_name: &str) -> bool {
        debug!(
            " 🧪 **** Default on_order_info called: order_info={:?}, order_name={}",
            order_info, order_name
        );
        true
    }
}
