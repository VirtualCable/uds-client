// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

pub mod ui;

pub const INTER_FONT_DATA: &[u8] = include_bytes!("fonts/Inter-Regular.ttf");

/// Load a PNG file (embedded via include_bytes!) and return RGBA pixel data + dimensions.
/// The alpha channel is preserved — transparent areas won't be drawn.
pub fn load_png_rgba(png_bytes: &[u8]) -> (Vec<u8>, u32, u32) {
    let img = image::load_from_memory(png_bytes).expect("Failed to decode PNG");
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    (rgba.into_raw(), w, h)
}

#[allow(dead_code)]
/// Generate a glass-style rounded rectangle RGBA buffer.
/// `color` = [r, g, b, a] for the fill.
pub fn glass_rect_rgba(w: u32, h: u32, color: [u8; 4]) -> Vec<u8> {
    let radius = (h.min(w) as f32 * 0.2).min(8.0) as u32;
    let mut buf = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let alpha = corner_alpha(x, y, w, h, radius);
            if alpha > 0.0 {
                let i = ((y * w + x) * 4) as usize;
                buf[i] = color[0];
                buf[i + 1] = color[1];
                buf[i + 2] = color[2];
                buf[i + 3] = ((color[3] as f32) * alpha).round() as u8;
            }
        }
    }
    buf
}

fn corner_alpha(x: u32, y: u32, w: u32, h: u32, r: u32) -> f32 {
    // Check if in corner region
    let in_left = x < r;
    let in_right = x >= w.saturating_sub(r);
    let in_top = y < r;
    let in_bottom = y >= h.saturating_sub(r);

    if in_left && in_top {
        let dx = r - x;
        let dy = r - y;
        let d = ((dx * dx + dy * dy) as f32).sqrt();
        if d > r as f32 {
            return 0.0;
        }
        ((r as f32 - d) / r as f32).clamp(0.0, 1.0)
    } else if in_right && in_top {
        let dx = x - (w - r);
        let dy = r - y;
        let d = ((dx * dx + dy * dy) as f32).sqrt();
        if d > r as f32 {
            return 0.0;
        }
        ((r as f32 - d) / r as f32).clamp(0.0, 1.0)
    } else if in_left && in_bottom {
        let dx = r - x;
        let dy = y - (h - r);
        let d = ((dx * dx + dy * dy) as f32).sqrt();
        if d > r as f32 {
            return 0.0;
        }
        ((r as f32 - d) / r as f32).clamp(0.0, 1.0)
    } else if in_right && in_bottom {
        let dx = x - (w - r);
        let dy = y - (h - r);
        let d = ((dx * dx + dy * dy) as f32).sqrt();
        if d > r as f32 {
            return 0.0;
        }
        ((r as f32 - d) / r as f32).clamp(0.0, 1.0)
    } else {
        1.0
    }
}
