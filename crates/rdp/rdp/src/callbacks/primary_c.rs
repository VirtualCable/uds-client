use freerdp_sys::{
    BOOL, DRAW_NINE_GRID_ORDER, DSTBLT_ORDER, ELLIPSE_CB_ORDER, ELLIPSE_SC_ORDER, FAST_GLYPH_ORDER,
    FAST_INDEX_ORDER, GLYPH_INDEX_ORDER, LINE_TO_ORDER, MEM3BLT_ORDER, MEMBLT_ORDER,
    MULTI_DRAW_NINE_GRID_ORDER, MULTI_DSTBLT_ORDER, MULTI_OPAQUE_RECT_ORDER, MULTI_PATBLT_ORDER,
    MULTI_SCRBLT_ORDER, OPAQUE_RECT_ORDER, ORDER_INFO, PATBLT_ORDER, POLYGON_CB_ORDER,
    POLYGON_SC_ORDER, POLYLINE_ORDER, SAVE_BITMAP_ORDER, SCRBLT_ORDER, rdpContext,
};

use crate::{
    callbacks::primary::PrimaryCallbacks, connection::context::OwnerFromCtx, utils::ToStringLossy,
};

use shared::log;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Callbacks {
    DstBlt,
    PatBlt,
    ScrBlt,
    OpaqueRect,
    DrawNineGrid,
    MultiDstBlt,
    MultiPatBlt,
    MultiScrBlt,
    MultiOpaqueRect,
    MultiDrawNineGrid,
    LineTo,
    Polyline,
    MemBlt,
    Mem3Blt,
    SaveBitmap,
    GlyphIndex,
    FastIndex,
    FastGlyph,
    PolygonSC,
    PolygonCB,
    EllipseSC,
    EllipseCB,
    OrderInfo,
}

impl Callbacks {
    #[allow(dead_code)]
    pub fn all() -> Vec<Callbacks> {
        vec![
            Callbacks::DstBlt,
            Callbacks::PatBlt,
            Callbacks::ScrBlt,
            Callbacks::OpaqueRect,
            Callbacks::DrawNineGrid,
            Callbacks::MultiDstBlt,
            Callbacks::MultiPatBlt,
            Callbacks::MultiScrBlt,
            Callbacks::MultiOpaqueRect,
            Callbacks::MultiDrawNineGrid,
            Callbacks::LineTo,
            Callbacks::Polyline,
            Callbacks::MemBlt,
            Callbacks::Mem3Blt,
            Callbacks::SaveBitmap,
            Callbacks::GlyphIndex,
            Callbacks::FastIndex,
            Callbacks::FastGlyph,
            Callbacks::PolygonSC,
            Callbacks::PolygonCB,
            Callbacks::EllipseSC,
            Callbacks::EllipseCB,
            Callbacks::OrderInfo,
        ]
    }
}

/// # Safety
/// Interoperability with C code.
/// Ensure that the context pointer is valid.
pub unsafe fn set_callbacks(context: *mut rdpContext, overrides: &[Callbacks]) {
    unsafe {
        let update = (*context).update;
        let primary = (*update).primary;
        if update.is_null() || primary.is_null() {
            log::debug!(" 🧪 **** Primary not initialized, cannot override callbacks.");
            return;
        }

        for override_cb in overrides {
            match override_cb {
                Callbacks::DstBlt => {
                    (*primary).DstBlt = Some(dst_blt);
                }
                Callbacks::PatBlt => {
                    (*primary).PatBlt = Some(pat_blt);
                }
                Callbacks::ScrBlt => {
                    (*primary).ScrBlt = Some(scr_blt);
                }
                Callbacks::OpaqueRect => {
                    (*primary).OpaqueRect = Some(opaque_rect);
                }
                Callbacks::DrawNineGrid => {
                    (*primary).DrawNineGrid = Some(draw_nine_grid);
                }
                Callbacks::MultiDstBlt => {
                    (*primary).MultiDstBlt = Some(multi_dst_blt);
                }
                Callbacks::MultiPatBlt => {
                    (*primary).MultiPatBlt = Some(multi_pat_blt);
                }
                Callbacks::MultiScrBlt => {
                    (*primary).MultiScrBlt = Some(multi_scr_blt);
                }
                Callbacks::MultiOpaqueRect => {
                    (*primary).MultiOpaqueRect = Some(multi_opaque_rect);
                }
                Callbacks::MultiDrawNineGrid => {
                    (*primary).MultiDrawNineGrid = Some(multi_draw_nine_grid);
                }
                Callbacks::LineTo => {
                    (*primary).LineTo = Some(line_to);
                }
                Callbacks::Polyline => {
                    (*primary).Polyline = Some(polyline);
                }
                Callbacks::MemBlt => {
                    (*primary).MemBlt = Some(mem_blt);
                }
                Callbacks::Mem3Blt => {
                    (*primary).Mem3Blt = Some(mem3_blt);
                }
                Callbacks::SaveBitmap => {
                    (*primary).SaveBitmap = Some(save_bitmap);
                }
                Callbacks::GlyphIndex => {
                    (*primary).GlyphIndex = Some(glyph_index);
                }
                Callbacks::FastIndex => {
                    (*primary).FastIndex = Some(fast_index);
                }
                Callbacks::FastGlyph => {
                    (*primary).FastGlyph = Some(fast_glyph);
                }
                Callbacks::PolygonSC => {
                    (*primary).PolygonSC = Some(polygon_sc);
                }
                Callbacks::PolygonCB => {
                    (*primary).PolygonCB = Some(polygon_cb);
                }
                Callbacks::EllipseSC => {
                    (*primary).EllipseSC = Some(ellipse_sc);
                }
                Callbacks::EllipseCB => {
                    (*primary).EllipseCB = Some(ellipse_cb);
                }
                Callbacks::OrderInfo => {
                    (*primary).OrderInfo = Some(order_info);
                }
            }
        }
    }
}

pub extern "C" fn dst_blt(context: *mut rdpContext, dstblt: *const DSTBLT_ORDER) -> BOOL {
    log::debug!(" 🧪 **** DST BLT called...");
    if let Some(owner) = context.owner() {
        owner.on_dst_blt(dstblt).into()
    } else {
        true.into()
    }
}

pub extern "C" fn pat_blt(context: *mut rdpContext, patblt: *mut PATBLT_ORDER) -> BOOL {
    log::debug!(" 🧪 **** PAT BLT called...");
    if let Some(owner) = context.owner() {
        owner.on_pat_blt(patblt).into()
    } else {
        true.into()
    }
}

pub extern "C" fn scr_blt(context: *mut rdpContext, scrblt: *const SCRBLT_ORDER) -> BOOL {
    log::debug!(" 🧪 **** SCR BLT called...");
    if let Some(owner) = context.owner() {
        owner.on_scr_blt(scrblt).into()
    } else {
        true.into()
    }
}

pub extern "C" fn opaque_rect(
    context: *mut rdpContext,
    opaque_rect: *const OPAQUE_RECT_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** OPAQUE RECT called...");
    if let Some(owner) = context.owner() {
        owner.on_opaque_rect(opaque_rect).into()
    } else {
        true.into()
    }
}

pub extern "C" fn draw_nine_grid(
    context: *mut rdpContext,
    draw_nine_grid: *const DRAW_NINE_GRID_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** DRAW NINE GRID called...");
    if let Some(owner) = context.owner() {
        owner.on_draw_nine_grid(draw_nine_grid).into()
    } else {
        true.into()
    }
}

pub extern "C" fn multi_dst_blt(
    context: *mut rdpContext,
    multi_dstblt: *const MULTI_DSTBLT_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** MULTI DST BLT called...");
    if let Some(owner) = context.owner() {
        owner.on_multi_dst_blt(multi_dstblt).into()
    } else {
        true.into()
    }
}

pub extern "C" fn multi_pat_blt(
    context: *mut rdpContext,
    multi_patblt: *const MULTI_PATBLT_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** MULTI PAT BLT called...");
    if let Some(owner) = context.owner() {
        owner.on_multi_pat_blt(multi_patblt).into()
    } else {
        true.into()
    }
}

pub extern "C" fn multi_scr_blt(
    context: *mut rdpContext,
    multi_scrblt: *const MULTI_SCRBLT_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** MULTI SCR BLT called...");
    if let Some(owner) = context.owner() {
        owner.on_multi_scr_blt(multi_scrblt).into()
    } else {
        true.into()
    }
}

pub extern "C" fn multi_opaque_rect(
    context: *mut rdpContext,
    multi_opaque_rect: *const MULTI_OPAQUE_RECT_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** MULTI OPAQUE RECT called...");
    if let Some(owner) = context.owner() {
        owner.on_multi_opaque_rect(multi_opaque_rect).into()
    } else {
        true.into()
    }
}

pub extern "C" fn multi_draw_nine_grid(
    context: *mut rdpContext,
    multi_draw_nine_grid: *const MULTI_DRAW_NINE_GRID_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** MULTI DRAW NINE GRID called...");
    if let Some(owner) = context.owner() {
        owner.on_multi_draw_nine_grid(multi_draw_nine_grid).into()
    } else {
        true.into()
    }
}

pub extern "C" fn line_to(context: *mut rdpContext, line_to: *const LINE_TO_ORDER) -> BOOL {
    log::debug!(" 🧪 **** LINE TO called...");
    if let Some(owner) = context.owner() {
        owner.on_line_to(line_to).into()
    } else {
        true.into()
    }
}

pub extern "C" fn polyline(context: *mut rdpContext, polyline: *const POLYLINE_ORDER) -> BOOL {
    log::debug!(" 🧪 **** POLYLINE called...");
    if let Some(owner) = context.owner() {
        owner.on_polyline(polyline).into()
    } else {
        true.into()
    }
}

pub extern "C" fn mem_blt(context: *mut rdpContext, memblt: *mut MEMBLT_ORDER) -> BOOL {
    log::debug!(" 🧪 **** MEM BLT called...");
    if let Some(owner) = context.owner() {
        owner.on_mem_blt(memblt).into()
    } else {
        true.into()
    }
}

pub extern "C" fn mem3_blt(context: *mut rdpContext, memblt: *mut MEM3BLT_ORDER) -> BOOL {
    log::debug!(" 🧪 **** MEM3 BLT called...");
    if let Some(owner) = context.owner() {
        owner.on_mem3_blt(memblt).into()
    } else {
        true.into()
    }
}

pub extern "C" fn save_bitmap(
    context: *mut rdpContext,
    bitmap_data: *const SAVE_BITMAP_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** SAVE BITMAP called...");
    if let Some(owner) = context.owner() {
        owner.on_save_bitmap(bitmap_data).into()
    } else {
        true.into()
    }
}

pub extern "C" fn glyph_index(
    context: *mut rdpContext,
    glyph_index: *mut GLYPH_INDEX_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** GLYPH INDEX called...");
    if let Some(owner) = context.owner() {
        owner.on_glyph_index(glyph_index).into()
    } else {
        true.into()
    }
}

pub extern "C" fn fast_index(context: *mut rdpContext, glyph: *const FAST_INDEX_ORDER) -> BOOL {
    log::debug!(" 🧪 **** FAST INDEX called...");
    if let Some(owner) = context.owner() {
        owner.on_fast_index(glyph).into()
    } else {
        true.into()
    }
}

pub extern "C" fn fast_glyph(context: *mut rdpContext, glyph: *const FAST_GLYPH_ORDER) -> BOOL {
    log::debug!(" 🧪 **** FAST GLYPH called...");
    if let Some(owner) = context.owner() {
        owner.on_fast_glyph(glyph).into()
    } else {
        true.into()
    }
}

pub extern "C" fn polygon_sc(
    context: *mut rdpContext,
    polygon_sc: *const POLYGON_SC_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** POLYGON SC called...");
    if let Some(owner) = context.owner() {
        owner.on_polygon_sc(polygon_sc).into()
    } else {
        true.into()
    }
}

pub extern "C" fn polygon_cb(context: *mut rdpContext, polygon_cb: *mut POLYGON_CB_ORDER) -> BOOL {
    log::debug!(" 🧪 **** POLYGON CB called...");
    if let Some(owner) = context.owner() {
        owner.on_polygon_cb(polygon_cb).into()
    } else {
        true.into()
    }
}

pub extern "C" fn ellipse_sc(
    context: *mut rdpContext,
    ellipse_sc: *const ELLIPSE_SC_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** ELLIPSE SC called...");
    if let Some(owner) = context.owner() {
        owner.on_ellipse_sc(ellipse_sc).into()
    } else {
        true.into()
    }
}

pub extern "C" fn ellipse_cb(
    context: *mut rdpContext,
    ellipse_cb: *const ELLIPSE_CB_ORDER,
) -> BOOL {
    log::debug!(" 🧪 **** ELLIPSE CB called...");
    if let Some(owner) = context.owner() {
        owner.on_ellipse_cb(ellipse_cb).into()
    } else {
        true.into()
    }
}

pub extern "C" fn order_info(
    context: *mut rdpContext,
    order_info: *const ORDER_INFO,
    order_name: *const ::std::os::raw::c_char,
) -> BOOL {
    log::debug!(" 🧪 **** ORDER INFO called...");
    if let Some(owner) = context.owner() {
        let order_name = order_name.to_string_lossy();
        owner.on_order_info(order_info, &order_name).into()
    } else {
        true.into()
    }
}
