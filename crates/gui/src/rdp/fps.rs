use std::sync::atomic::{AtomicBool, Ordering};

use crate::monitor;

#[allow(dead_code)]
pub struct Fps {
    pub last_instant: std::time::Instant,
    frames: Vec<std::time::Instant>,
    pub enabled: AtomicBool,
    bg_rgba: Vec<u8>,
    bg_w: u32,
    bg_h: u32,
}

impl Fps {
    pub fn new() -> Self {
        let (bg_rgba, bw, bh) =
            crate::draw::load_png_rgba(include_bytes!("../images/fps.png"));
        Self {
            last_instant: std::time::Instant::now(),
            frames: Vec::new(),
            enabled: AtomicBool::new(false),
            bg_rgba,
            bg_w: bw,
            bg_h: bh,
        }
    }
    pub fn record(&mut self) {
        let now = std::time::Instant::now();
        self.frames
            .retain(|t| now.duration_since(*t).as_secs_f32() < 2.0);
        self.frames.push(now);
    }
    pub fn toggle(&self) {
        let v = self.enabled.load(Ordering::Relaxed);
        self.enabled.store(!v, Ordering::Relaxed);
    }
    pub fn average(&self) -> f32 {
        let now = std::time::Instant::now();
        let recent: Vec<_> = self
            .frames
            .iter()
            .filter(|t| now.duration_since(**t).as_secs_f32() < 1.0)
            .collect();
        recent.len() as f32
    }

    pub fn build_overlay(
        &self,
        phys_w: u32,
        text_sections: &mut Vec<crate::wgpu_render::OwnedSection>,
        ov_data: &mut Vec<Vec<u8>>,
    ) -> Option<crate::wgpu_render::OverlayDesc> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let bg_w = monitor::scaled_val((self.bg_w as i32 / 2).max(1)) as u32;
        let margin_y = monitor::scaled_val(0) as u32;
        let margin_x = monitor::scaled_val(24) as u32;
        let x = phys_w.saturating_sub(bg_w + margin_x) as f32;
        let y = margin_y as f32;
        let idx = ov_data.len();
        ov_data.push(self.bg_rgba.clone());
        let scale = bg_w as f32 / self.bg_w as f32;

        let fps_text = format!("{:.0}", self.average());
        let font_size = monitor::scaled_val(12) as f32;
        text_sections.push(
            crate::wgpu_render::Section::default()
                .add_text(
                    crate::wgpu_render::Text::new(&fps_text)
                        .with_scale(font_size)
                        .with_color([1.0, 1.0, 1.0, 1.0]),
                )
                .with_screen_position((
                    x + monitor::scaled_val(26) as f32,
                    y + monitor::scaled_val(2) as f32,
                ))
                .to_owned(),
        );

        Some(crate::wgpu_render::OverlayDesc {
            data_idx: idx,
            w: self.bg_w,
            h: self.bg_h,
            x,
            y,
            scale,
        })
    }
}
