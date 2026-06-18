// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use crate::monitor;

#[allow(dead_code)]
pub struct Pinbar {
    pub visible: bool,
    pub rect: Option<(u32, u32)>,
    pub btn_fs_x: std::ops::Range<f32>,
    pub btn_close_x: std::ops::Range<f32>,
    bg_rgba: Vec<u8>,
    bg_w: u32,
    bg_h: u32,
}

impl Default for Pinbar {
    fn default() -> Self {
        Self::new()
    }
}

impl Pinbar {
    pub fn new() -> Self {
        let (bg_rgba, bw, bh) =
            crate::draw::load_png_rgba(include_bytes!("../../images/pinbar.png"));
        Self {
            visible: false,
            rect: None,
            btn_fs_x: 0.0..0.0,
            btn_close_x: 0.0..0.0,
            bg_rgba,
            bg_w: bw,
            bg_h: bh,
        }
    }

    pub fn build(
        &mut self,
        pw: u32,
        text_sections: &mut Vec<crate::wgpu_render::OwnedSection>,
        ov_data: &mut Vec<Vec<u8>>,
    ) -> Option<crate::wgpu_render::OverlayDesc> {
        if !self.visible {
            return None;
        }
        let bg_w = monitor::scaled_val(self.bg_w as i32) as u32;
        let bg_h = monitor::scaled_val(self.bg_h as i32) as u32;
        let x = (pw.saturating_sub(bg_w) / 2) as f32;
        let scale = *monitor::SCALE_FACTOR as f32;
        let data_idx = ov_data.len();
        ov_data.push(self.bg_rgba.clone());

        let font_size = monitor::scaled_val(16) as f32;
        text_sections.push(
            crate::wgpu_render::Section::default()
                .add_text(
                    crate::wgpu_render::Text::new("UDS Connection")
                        .with_scale(font_size)
                        .with_color([1.0, 1.0, 1.0, 1.0]),
                )
                .with_screen_position((
                    x + monitor::scaled_val(8) as f32,
                    monitor::scaled_val(8) as f32,
                ))
                .to_owned(),
        );

        self.btn_fs_x =
            (x + monitor::scaled_val(220) as f32)..(x + monitor::scaled_val(239) as f32);
        self.btn_close_x =
            (x + monitor::scaled_val(243) as f32)..(x + monitor::scaled_val(262) as f32);
        self.rect = Some((bg_w, bg_h));

        Some(crate::wgpu_render::OverlayDesc {
            data_idx,
            w: self.bg_w,
            h: self.bg_h,
            x,
            y: 0.0,
            scale,
        })
    }
}
