#![allow(dead_code)]
use eframe::egui;

pub use rdp::geom::Rect;

pub trait RectExt {
    fn extract(
        &self,
        framebuffer: &[u8],
        stride_bytes: usize,
        fb_width: usize,
        fb_height: usize,
    ) -> Option<egui::ColorImage>;
}

impl RectExt for Rect {
    fn extract(
        &self,
        framebuffer: &[u8],
        stride_bytes: usize,
        fb_width: usize,
        fb_height: usize,
    ) -> Option<egui::ColorImage> {
        if self.x > fb_width as u32 || self.y > fb_height as u32 {
            return None;
        }
        if self.x + self.w > fb_width as u32 || self.y + self.h > fb_height as u32 {
            return None;
        }
        let mut pixels = Vec::with_capacity(self.w as usize * self.h as usize);
        for row in 0..self.h as usize {
            let src_offset = (self.y as usize + row) * stride_bytes + self.x as usize * 4;
            let row_slice = &framebuffer[src_offset..src_offset + self.w as usize * 4];
            for px in row_slice.chunks_exact(4) {
                pixels.push(egui::Color32::from_rgba_unmultiplied(
                    px[2], px[1], px[0], px[3],
                )); // RGBA
            }
        }
        Some(egui::ColorImage {
            size: [self.w as usize, self.h as usize],
            source_size: egui::Vec2::new(fb_width as f32, fb_height as f32),
            pixels,
        })
    }
}
