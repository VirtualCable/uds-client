// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

pub struct Cursor {
    pub data: Vec<u8>,
    pub hot_x: u32,
    pub hot_y: u32,
    pub w: u32,
    pub h: u32,
    pub visible: bool,
    pub x: f32,
    pub y: f32,
    pub scale: f64,
    pub dirty: bool,
    pub use_rgba: bool,
}

impl Cursor {
    pub fn new(cursor_scale: f64, use_rgba: bool) -> Self {
        Self {
            data: Vec::new(),
            hot_x: 0,
            hot_y: 0,
            w: 0,
            h: 0,
            visible: false,
            x: 0.0,
            y: 0.0,
            scale: cursor_scale,
            dirty: false,
            use_rgba,
        }
    }

    pub fn set_icon(&mut self, mut data: Vec<u8>, x: u32, y: u32, width: u32, height: u32) {
        if !self.use_rgba {
            // Convert BGRA to RGBA by swapping Blue and Red channels
            for chunk in data.chunks_exact_mut(4) {
                chunk.swap(0, 2);
            }
        }
        self.data = data;
        self.hot_x = x;
        self.hot_y = y;
        self.w = width;
        self.h = height;
        self.visible = width > 0 && height > 0;
        self.dirty = true;
    }

    pub fn build_overlay(&self) -> Option<crate::wgpu_render::OverlayParams<'_>> {
        if !self.visible || self.data.is_empty() {
            return None;
        }
        let (hot_x, hot_y) =
            crate::monitor::logic_2_phys_pos((self.hot_x as i32, self.hot_y as i32), self.scale);
        Some(crate::wgpu_render::OverlayParams {
            rgba: self.data.as_slice(),
            width: self.w,
            height: self.h,
            x: self.x - hot_x as f32,
            y: self.y - hot_y as f32,
            scale: self.scale as f32,
        })
    }

    pub fn build_scaled_rgba(&self) -> (Vec<u8>, u16, u16, u16, u16) {
        let scale = self.scale;
        if scale > 1.001 {
            let sw = (self.w as f64 * scale).round() as u32;
            let sh = (self.h as f64 * scale).round() as u32;
            let hx = (self.hot_x as f64 * scale).round() as u32;
            let hy = (self.hot_y as f64 * scale).round() as u32;

            let mut scaled = Vec::with_capacity((sw * sh * 4) as usize);
            for y in 0..sh {
                let src_y = (y * self.h) / sh;
                for x in 0..sw {
                    let src_x = (x * self.w) / sw;
                    let src_idx = ((src_y * self.w + src_x) * 4) as usize;
                    if src_idx + 3 < self.data.len() {
                        scaled.extend_from_slice(&self.data[src_idx..src_idx + 4]);
                    } else {
                        scaled.extend_from_slice(&[0, 0, 0, 0]);
                    }
                }
            }
            (scaled, sw as u16, sh as u16, hx as u16, hy as u16)
        } else {
            (
                self.data.clone(),
                self.w as u16,
                self.h as u16,
                self.hot_x as u16,
                self.hot_y as u16,
            )
        }
    }
}
