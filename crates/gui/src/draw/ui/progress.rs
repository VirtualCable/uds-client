// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

/// Render a progress bar into an RGBA buffer.
/// `pct` is 0.0–100.0.
pub fn render(pct: f32, w: u32, h: u32) -> Vec<u8> {
    let mut pixmap = Pixmap::new(w, h).unwrap();
    let s = h as f32 / 16.0; // Assume 16px is base height

    // Background (dark track)
    let bg_path = rounded_rect(0.0, 0.0, w as f32, h as f32, h as f32 / 2.0);
    let mut bg_paint = Paint::default();
    bg_paint.set_color(Color::from_rgba8(0x25, 0x25, 0x35, 0xFF));
    pixmap.fill_path(&bg_path, &bg_paint, FillRule::Winding, Transform::identity(), None);

    // Track Border
    let mut border_paint = Paint::default();
    border_paint.set_color(Color::from_rgba8(0x45, 0x45, 0x55, 0xFF));
    let stroke = tiny_skia::Stroke {
        width: 1.0 * s,
        ..Default::default()
    };
    pixmap.stroke_path(&bg_path, &border_paint, &stroke, Transform::identity(), None);

    // Filled portion
    let fw = (w as f32 * pct / 100.0).clamp(0.0, w as f32);
    if fw > (h as f32 / 2.0) {
        let fill_path = rounded_rect(1.0 * s, 1.0 * s, fw - 2.0 * s, h as f32 - 2.0 * s, (h as f32 - 2.0 * s) / 2.0);
        
        // Gradient fill
        let mut fill_paint = Paint::default();
        let grad = tiny_skia::LinearGradient::new(
            tiny_skia::Point::from_xy(0.0, 0.0),
            tiny_skia::Point::from_xy(0.0, h as f32),
            vec![
                tiny_skia::GradientStop::new(0.0, Color::from_rgba8(0x60, 0xC0, 0xFF, 0xFF)),
                tiny_skia::GradientStop::new(1.0, Color::from_rgba8(0x20, 0x80, 0xE0, 0xFF)),
            ],
            tiny_skia::SpreadMode::Pad,
            Transform::identity(),
        ).unwrap();
        fill_paint.shader = grad;
        
        pixmap.fill_path(&fill_path, &fill_paint, FillRule::Winding, Transform::identity(), None);
    }

    pixmap.take()
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(
        x + w - r + r * 0.552,
        y,
        x + w,
        y + r - r * 0.552,
        x + w,
        y + r,
    );
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(
        x + w,
        y + h - r + r * 0.552,
        x + w - r + r * 0.552,
        y + h,
        x + w - r,
        y + h,
    );
    pb.line_to(x + r, y + h);
    pb.cubic_to(
        x + r - r * 0.552,
        y + h,
        x,
        y + h - r + r * 0.552,
        x,
        y + h - r,
    );
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - r * 0.552, x + r - r * 0.552, y, x + r, y);
    pb.finish().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_output_dimensions() {
        let buf = render(50.0, 100, 20);
        assert_eq!(buf.len(), (100 * 20 * 4) as usize);
    }

    #[test]
    fn render_zero_pct() {
        let buf = render(0.0, 50, 10);
        assert_eq!(buf.len(), (50 * 10 * 4) as usize);
    }

    #[test]
    fn render_full_pct() {
        let buf = render(100.0, 50, 10);
        assert_eq!(buf.len(), (50 * 10 * 4) as usize);
    }

    #[test]
    fn render_negative_clamped() {
        let buf = render(-10.0, 50, 10);
        assert_eq!(buf.len(), (50 * 10 * 4) as usize);
    }
}
