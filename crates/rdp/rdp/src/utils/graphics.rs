// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

use crate::geom::Rect;
use crate::utils::log;
use freerdp_sys::GDI_RGN;

pub fn pixel_format(bpp: u8, pixel_type: u8, a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((bpp as u32) << 24)
        | ((pixel_type as u32) << 16)
        | ((a as u32) << 12)
        | ((r as u32) << 8)
        | ((g as u32) << 4)
        | (b as u32)
}

pub fn normalize_rects(rects_raw: &[GDI_RGN], width: u32, height: u32) -> Option<Vec<Rect>> {
    let width = width as i32;
    let height = height as i32;
    rects_raw
        .iter()
        .filter_map(|r| {
            if r.x <= width
                && r.y < height
                && r.x >= 0
                && r.y >= 0
                && r.w <= width
                && r.w > 0
                && r.h <= height
                && r.h > 0
            {
                #[allow(clippy::unnecessary_cast)]
                Some(Rect {
                    x: r.x as i32,
                    y: r.y as i32,
                    w: r.w as u32,
                    h: r.h as u32,
                })
            } else {
                log::debug!("Skipping invalid rect: {:?}", r);
                None
            }
        })
        .collect::<Vec<_>>()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_format_rgba32() {
        let pf = pixel_format(32, 3, 8, 8, 8, 8);
        assert_eq!(pf, 0x20038888);
    }

    #[test]
    fn pixel_format_bgra32() {
        let pf = pixel_format(32, 4, 8, 8, 8, 8);
        assert_eq!(pf, 0x20048888);
    }

    #[test]
    fn pixel_format_all_zero() {
        assert_eq!(pixel_format(0, 0, 0, 0, 0, 0), 0);
    }
}
