#[allow(dead_code)]
pub struct Pinbar {
    pub visible: bool,
    pub rect: Option<(u32, u32)>,
    pub btn_fs_x: std::ops::Range<f32>,
    pub btn_close_x: std::ops::Range<f32>,
}

impl Pinbar {
    pub fn new() -> Self {
        Self {
            visible: false,
            rect: None,
            btn_fs_x: 0.0..0.0,
            btn_close_x: 0.0..0.0,
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
        let pinbar_bg = include_bytes!("../images/pinbar.png");
        let (bg_rgba, bw, bh) = crate::draw::load_png_rgba(pinbar_bg);
        let bg_w = crate::monitor::scaled_val(bw as i32) as u32;
        let bg_h = crate::monitor::scaled_val(bh as i32) as u32;
        let x = (pw.saturating_sub(bg_w) / 2) as f32;
        let scale = *crate::monitor::SCALE_FACTOR as f32;
        let data_idx = ov_data.len();
        ov_data.push(bg_rgba);

        let font_size = crate::monitor::scaled_val(16) as f32;
        text_sections.push(
            crate::wgpu_render::Section::default()
                .add_text(
                    crate::wgpu_render::Text::new("UDS Connection")
                        .with_scale(font_size)
                        .with_color([1.0, 1.0, 1.0, 1.0]),
                )
                .with_screen_position((
                    x + crate::monitor::scaled_val(8) as f32,
                    crate::monitor::scaled_val(8) as f32,
                ))
                .to_owned(),
        );

        self.btn_fs_x =
            (x + crate::monitor::scaled_val(220) as f32)..(x + crate::monitor::scaled_val(239) as f32);
        self.btn_close_x =
            (x + crate::monitor::scaled_val(243) as f32)..(x + crate::monitor::scaled_val(262) as f32);
        self.rect = Some((bg_w, bg_h));

        Some(crate::wgpu_render::OverlayDesc {
            data_idx,
            w: bw,
            h: bh,
            x,
            y: 0.0,
            scale,
        })
    }
}
