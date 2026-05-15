// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

/// Scale an RGBA image buffer by a factor, returning the resized RGBA buffer
/// and its new dimensions.
#[allow(dead_code)]
pub fn scale(rgba: &[u8], src_w: u32, src_h: u32, scale: f32) -> (Vec<u8>, u32, u32) {
    if (scale - 1.0).abs() < f32::EPSILON {
        return (rgba.to_vec(), src_w, src_h);
    }
    let img =
        image::RgbaImage::from_raw(src_w, src_h, rgba.to_vec()).expect("Invalid image dimensions");
    let dst_w = (src_w as f32 * scale).round() as u32;
    let dst_h = (src_h as f32 * scale).round() as u32;
    let resized =
        image::imageops::resize(&img, dst_w, dst_h, image::imageops::FilterType::Lanczos3);
    (resized.into_raw(), dst_w, dst_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_rgba(w: u32, h: u32, r: u8, g: u8, b: u8, a: u8) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            v.extend_from_slice(&[r, g, b, a]);
        }
        v
    }

    #[test]
    fn scale_identity() {
        let src = solid_rgba(4, 4, 255, 0, 0, 255);
        let (out, w, h) = scale(&src, 4, 4, 1.0);
        assert_eq!((w, h), (4, 4));
        assert_eq!(out.len(), 4 * 4 * 4);
        assert_eq!(out, src);
    }

    #[test]
    fn scale_up_2x() {
        let src = solid_rgba(4, 4, 255, 0, 0, 255);
        let (out, w, h) = scale(&src, 4, 4, 2.0);
        assert_eq!((w, h), (8, 8));
        assert_eq!(out.len(), 8 * 8 * 4);
    }

    #[test]
    fn scale_down_half() {
        let src = solid_rgba(8, 8, 0, 255, 0, 255);
        let (out, w, h) = scale(&src, 8, 8, 0.5);
        assert_eq!((w, h), (4, 4));
        assert_eq!(out.len(), 4 * 4 * 4);
    }
}
