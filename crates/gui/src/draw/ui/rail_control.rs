// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use crate::draw::ui::button;
use crate::wgpu_render::{OverlayParams, WgpuRenderer};
use tiny_skia::{Color, FillRule, Paint, Pixmap, Stroke, Transform};
use wgpu_text::glyph_brush::{OwnedSection, Section, Text};

pub struct RailControl {
    pub buttons: Vec<crate::draw::ui::button::Button>,
    pub text: String,
}

impl RailControl {
    pub fn new(text: String, width: f32, height: f32, scale: f32, exit_text: String) -> Self {
        let mut buttons = Vec::new();
        let bw = crate::monitor::scaled_val(60) as f32;
        let bh = crate::monitor::scaled_val(25) as f32;
        let bx = width - bw - 10.0 * scale;
        let by = height / 2.0 - bh / 2.0;

        buttons.push(crate::draw::ui::button::Button::new(
            bx,
            by,
            bw as u32,
            bh as u32,
            exit_text,
            crate::draw::ui::button::ButtonStyle {
                font_scale: crate::monitor::scaled_val(12) as f32,
                radius: 4.0,
                bg_color: [180, 40, 40, 255], // Reddish for exit
                border_color: [200, 60, 60, 255],
                hover_bg_color: [220, 50, 50, 255],
                hover_border_color: [255, 100, 100, 255],
                ..Default::default()
            },
        ));

        Self { buttons, text }
    }

    pub fn handle_mouse_move(&mut self, x: f32, y: f32) -> bool {
        let mut changed = false;
        for btn in &mut self.buttons {
            if btn.handle_mouse_move(x, y) {
                changed = true;
            }
        }
        changed
    }

    pub fn handle_click(&mut self, x: f32, y: f32) -> bool {
        for btn in &self.buttons {
            if btn.contains(x, y) {
                return true; // Clicked exit
            }
        }
        false
    }

    pub fn paint(&mut self, renderer: &mut WgpuRenderer, pw: u32, ph: u32, scale: f32) {
        renderer.reconfigure(pw, ph);

        let mut sections: Vec<OwnedSection> = Vec::new();
        let mut data: Vec<Vec<u8>> = Vec::new();
        let mut ov_descs: Vec<(usize, u32, u32, f32, f32)> = Vec::new();

        // 1. Draw Background Panel
        let mut panel_pixmap = Pixmap::new(pw, ph).unwrap();
        let rect =
            button::rounded_rect_path(1.0, 1.0, pw as f32 - 2.0, ph as f32 - 2.0, 6.0 * scale);

        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(30, 30, 35, 255));
        panel_pixmap.fill_path(
            &rect,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        let stroke = Stroke {
            width: 1.0 * scale,
            ..Default::default()
        };
        paint.set_color(Color::from_rgba8(80, 80, 100, 255));
        panel_pixmap.stroke_path(&rect, &paint, &stroke, Transform::identity(), None);

        data.push(panel_pixmap.take());
        ov_descs.push((0, pw, ph, 0.0, 0.0));

        // 2. Texts
        let text_fs = crate::monitor::scaled_val(14) as f32;
        sections.push(
            Section::default()
                .add_text(
                    Text::new(&self.text)
                        .with_scale(text_fs)
                        .with_color([1.0, 1.0, 1.0, 1.0]),
                )
                .with_screen_position((15.0 * scale, ph as f32 / 2.0))
                .with_layout(
                    wgpu_text::glyph_brush::Layout::default()
                        .v_align(wgpu_text::glyph_brush::VerticalAlign::Center),
                )
                .to_owned(),
        );

        // 3. Buttons
        for btn in &self.buttons {
            let (btn_data, btn_text) = btn.render();
            data.push(btn_data);
            ov_descs.push((data.len() - 1, btn.w, btn.h, btn.x, btn.y));
            sections.push(btn_text);
        }

        let mut overlays = Vec::with_capacity(ov_descs.len());
        for (di, w, h, x, y) in &ov_descs {
            overlays.push(OverlayParams {
                rgba: &data[*di],
                width: *w,
                height: *h,
                x: *x,
                y: *y,
                scale: 1.0,
            });
        }

        renderer.update_and_render(&[], pw, ph, &overlays, &sections, None, None);
    }
}
