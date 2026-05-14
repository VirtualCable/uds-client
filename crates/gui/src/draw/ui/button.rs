// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};

pub struct ButtonStyle {
    pub bg_color: [u8; 4],
    pub border_color: [u8; 4],
    pub radius: f32,
    pub font_scale: f32,
    pub text_color: [f32; 4],
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            bg_color: [0x50, 0x50, 0x70, 0xFF],
            border_color: [0x70, 0x70, 0x90, 0xFF],
            radius: 6.0,
            font_scale: 14.0,
            text_color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// Render a rounded-rect button into an RGBA buffer, returning the pixel data
/// and a wgpu_text `OwnedSection` for the centered label.
/// `x`, `y` — screen position where the button overlay will be placed.
pub fn render(
    x: f32,
    y: f32,
    w: u32,
    h: u32,
    label: &str,
    style: &ButtonStyle,
) -> (Vec<u8>, OwnedSection) {
    let mut pixmap = Pixmap::new(w, h).unwrap();

    // Rounded rect path
    let rect_path = rounded_rect_path(0.0, 0.0, w as f32, h as f32, style.radius);

    // Fill
    let mut fill = Paint::default();
    fill.set_color(Color::from_rgba8(
        style.bg_color[0],
        style.bg_color[1],
        style.bg_color[2],
        style.bg_color[3],
    ));
    pixmap.fill_path(
        &rect_path,
        &fill,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    // Border
    let mut stroke_paint = Paint::default();
    stroke_paint.set_color(Color::from_rgba8(
        style.border_color[0],
        style.border_color[1],
        style.border_color[2],
        style.border_color[3],
    ));
    let stroke = Stroke {
        width: 1.5,
        ..Default::default()
    };
    pixmap.stroke_path(&rect_path, &stroke_paint, &stroke, Transform::identity(), None);

    // Text section — positioned relative to button screen position
    let tx = x + w as f32 * 0.1;
    let ty = y + h as f32 * 0.15;
    let section = Section::default()
        .add_text(
            Text::new(label)
                .with_scale(style.font_scale)
                .with_color(style.text_color),
        )
        .with_screen_position((tx, ty))
        .to_owned();

    (pixmap.take(), section)
}

pub fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    // Top-right corner
    pb.cubic_to(
        x + w - r + r * 0.552, y,
        x + w, y + r - r * 0.552,
        x + w, y + r,
    );
    pb.line_to(x + w, y + h - r);
    // Bottom-right corner
    pb.cubic_to(
        x + w, y + h - r + r * 0.552,
        x + w - r + r * 0.552, y + h,
        x + w - r, y + h,
    );
    pb.line_to(x + r, y + h);
    // Bottom-left corner
    pb.cubic_to(
        x + r - r * 0.552, y + h,
        x, y + h - r + r * 0.552,
        x, y + h - r,
    );
    pb.line_to(x, y + r);
    // Top-left corner
    pb.cubic_to(x, y + r - r * 0.552, x + r - r * 0.552, y, x + r, y);
    pb.finish().unwrap()
}
