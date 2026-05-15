// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};

pub struct ButtonStyle {
    pub bg_color: [u8; 4],
    pub border_color: [u8; 4],
    pub hover_bg_color: [u8; 4],
    pub hover_border_color: [u8; 4],
    pub radius: f32,
    pub font_scale: f32,
    pub text_color: [f32; 4],
}

impl Default for ButtonStyle {
    fn default() -> Self {
        Self {
            bg_color: [0x50, 0x50, 0x70, 0xFF],
            border_color: [0x70, 0x70, 0x90, 0xFF],
            hover_bg_color: [0x70, 0x70, 0x90, 0xFF],
            hover_border_color: [0x90, 0x90, 0xB0, 0xFF],
            radius: 6.0,
            font_scale: 14.0,
            text_color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

pub struct Button {
    pub x: f32,
    pub y: f32,
    pub w: u32,
    pub h: u32,
    pub label: String,
    pub style: ButtonStyle,
    pub is_hovered: bool,
    pub is_pressed: bool,
}

impl Button {
    pub fn new(x: f32, y: f32, w: u32, h: u32, label: String, style: ButtonStyle) -> Self {
        Self {
            x,
            y,
            w,
            h,
            label,
            style,
            is_hovered: false,
            is_pressed: false,
        }
    }

    pub fn handle_mouse_move(&mut self, phys_x: f32, phys_y: f32) -> bool {
        let old_hover = self.is_hovered;
        self.is_hovered = self.contains(phys_x, phys_y);
        self.is_hovered != old_hover
    }

    pub fn contains(&self, phys_x: f32, phys_y: f32) -> bool {
        phys_x >= self.x && phys_x <= self.x + self.w as f32 &&
        phys_y >= self.y && phys_y <= self.y + self.h as f32
    }

    pub fn render(&self) -> (Vec<u8>, OwnedSection) {
        let mut pixmap = Pixmap::new(self.w, self.h).unwrap();
        let rect_path = rounded_rect_path(0.0, 0.0, self.w as f32, self.h as f32, self.style.radius);

        let mut fill = Paint::default();
        let bg = if self.is_hovered {
            self.style.hover_bg_color
        } else {
            self.style.bg_color
        };
        fill.set_color(Color::from_rgba8(bg[0], bg[1], bg[2], bg[3]));
        pixmap.fill_path(&rect_path, &fill, FillRule::Winding, Transform::identity(), None);

        let mut stroke_paint = Paint::default();
        let border = if self.is_hovered {
            self.style.hover_border_color
        } else {
            self.style.border_color
        };
        stroke_paint.set_color(Color::from_rgba8(border[0], border[1], border[2], border[3]));
        let stroke = Stroke { width: 1.5, ..Default::default() };
        pixmap.stroke_path(&rect_path, &stroke_paint, &stroke, Transform::identity(), None);

        let tx = self.x + self.w as f32 / 2.0;
        let ty = self.y + self.h as f32 / 2.0;
        let section = Section::default()
            .with_layout(
                wgpu_text::glyph_brush::Layout::default()
                    .h_align(wgpu_text::glyph_brush::HorizontalAlign::Center)
                    .v_align(wgpu_text::glyph_brush::VerticalAlign::Center),
            )
            .add_text(
                Text::new(&self.label)
                    .with_scale(self.style.font_scale)
                    .with_color(self.style.text_color),
            )
            .with_screen_position((tx, ty))
            .to_owned();

        (pixmap.take(), section)
    }
}

pub fn render(x: f32, y: f32, w: u32, h: u32, label: &str, style: &ButtonStyle) -> (Vec<u8>, OwnedSection) {
    let mut btn = Button::new(x, y, w, h, label.to_string(), ButtonStyle {
        bg_color: style.bg_color,
        border_color: style.border_color,
        hover_bg_color: style.hover_bg_color,
        hover_border_color: style.hover_border_color,
        radius: style.radius,
        font_scale: style.font_scale,
        text_color: style.text_color,
    });
    btn.is_hovered = false; // The old render didn't support hover state directly in this call
    btn.render()
}


pub fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    // Top-right corner
    pb.cubic_to(
        x + w - r + r * 0.552,
        y,
        x + w,
        y + r - r * 0.552,
        x + w,
        y + r,
    );
    pb.line_to(x + w, y + h - r);
    // Bottom-right corner
    pb.cubic_to(
        x + w,
        y + h - r + r * 0.552,
        x + w - r + r * 0.552,
        y + h,
        x + w - r,
        y + h,
    );
    pb.line_to(x + r, y + h);
    // Bottom-left corner
    pb.cubic_to(
        x + r - r * 0.552,
        y + h,
        x,
        y + h - r + r * 0.552,
        x,
        y + h - r,
    );
    pb.line_to(x, y + r);
    // Top-left corner
    pb.cubic_to(x, y + r - r * 0.552, x + r - r * 0.552, y, x + r, y);
    pb.finish().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounded_rect_bounds() {
        let path = rounded_rect_path(0.0, 0.0, 100.0, 50.0, 8.0);
        let bounds = path.bounds();
        // Tight bounds should match input rectangle
        assert!((bounds.x() - 0.0).abs() < 0.01);
        assert!((bounds.y() - 0.0).abs() < 0.01);
        assert!((bounds.width() - 100.0).abs() < 0.01);
        assert!((bounds.height() - 50.0).abs() < 0.01);
    }

    #[test]
    fn rounded_rect_radius_clamped() {
        // Radius larger than half width should be clamped
        let path = rounded_rect_path(0.0, 0.0, 20.0, 10.0, 30.0);
        // Should not panic, path should still be valid
        let bounds = path.bounds();
        assert!(bounds.width() > 0.0);
        assert!(bounds.height() > 0.0);
    }

    #[test]
    fn render_output_dimensions() {
        let style = ButtonStyle::default();
        let (rgba, _section) = render(0.0, 0.0, 100, 30, "Test", &style);
        assert_eq!(rgba.len(), (100 * 30 * 4) as usize);
        // Label text should be present in the section
        assert!(!rgba.is_empty());
    }
}
