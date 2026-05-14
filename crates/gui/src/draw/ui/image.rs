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
    let img = image::RgbaImage::from_raw(src_w, src_h, rgba.to_vec())
        .expect("Invalid image dimensions");
    let dst_w = (src_w as f32 * scale).round() as u32;
    let dst_h = (src_h as f32 * scale).round() as u32;
    let resized = image::imageops::resize(
        &img,
        dst_w,
        dst_h,
        image::imageops::FilterType::Lanczos3,
    );
    (resized.into_raw(), dst_w, dst_h)
}
