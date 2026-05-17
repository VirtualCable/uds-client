use anyhow::Result;
use shared::log;

use super::RdpState;
use crate::monitor;

fn rect_contains(outer: &rdp_ffi::geom::Rect, inner: &rdp_ffi::geom::Rect) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && (inner.x + inner.w as i32) <= (outer.x + outer.w as i32)
        && (inner.y + inner.h as i32) <= (outer.y + outer.h as i32)
}

fn rect_intersects(r1: &rdp_ffi::geom::Rect, r2: &rdp_ffi::geom::Rect) -> bool {
    r1.x < r2.x + r2.w as i32
        && r1.x + r1.w as i32 > r2.x
        && r1.y < r2.y + r2.h as i32
        && r1.y + r1.h as i32 > r2.y
}

fn rect_area(r: &rdp_ffi::geom::Rect) -> u32 {
    r.w * r.h
}

fn merge_rects(mut rects: Vec<rdp_ffi::geom::Rect>) -> Vec<rdp_ffi::geom::Rect> {
    if rects.len() <= 1 {
        return rects;
    }

    if rects.len() > 500 {
        let mut joined = rects[0];
        for r in &rects[1..] {
            joined = joined.union(r);
        }
        return vec![joined];
    }

    // 1. Deduplicate
    if rects.len() > 1 {
        let mut i = 0;
        while i < rects.len() {
            let mut j = 0;
            let mut removed = false;
            while j < rects.len() {
                if i != j && rect_contains(&rects[j], &rects[i]) {
                    rects.remove(i);
                    removed = true;
                    break;
                }
                j += 1;
            }
            if !removed {
                i += 1;
            }
        }
    }

    let mut changed = true;
    while changed && rects.len() > 1 {
        changed = false;
        let mut i = 0;
        while i < rects.len() {
            let mut j = i + 1;
            while j < rects.len() {
                let r1 = rects[i];
                let r2 = rects[j];

                let mut should_merge = rect_intersects(&r1, &r2);
                if !should_merge {
                    let union = r1.union(&r2);
                    if rect_area(&union)
                        < (rect_area(&r1) + rect_area(&r2)).saturating_mul(115) / 100
                    {
                        should_merge = true;
                    }
                }

                if should_merge {
                    rects[i] = r1.union(&r2);
                    rects.remove(j);
                    changed = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }

    const MAX_RECTS_PER_UPDATE: usize = 20;
    if rects.len() > MAX_RECTS_PER_UPDATE {
        let mut joined = rects[0];
        for r in &rects[1..] {
            joined = joined.union(r);
        }
        return vec![joined];
    }

    rects
}

impl RdpState {
    pub fn update_screen(&mut self) -> Result<()> {
        let gdi = self.gdi;

        let _gdi_guard = self.gdi_lock.read().unwrap();

        let (stride, fb_h, fb_w) = unsafe {
            (
                (*gdi).stride as usize,
                (*gdi).height as usize,
                (*gdi).width as usize,
            )
        };

        if fb_w == 0 || fb_h == 0 {
            log::warn!("update_screen: GDI dimensions are 0, skipping");
            return Ok(());
        }

        // Drain pending rects and merge them
        let rects = std::mem::take(&mut self.pendings.rects);
        let upload_rects = if !rects.is_empty() {
            let merged_rects = merge_rects(rects);

            let framebuffer = unsafe {
                std::slice::from_raw_parts((*gdi).primary_buffer as *const u8, stride * fb_h)
            };

            // Convert merged rects to (u32, u32, u32, u32) and clamp to framebuffer bounds
            let mut upload_rects = Vec::with_capacity(merged_rects.len());
            let mut total_size = 0;
            for r in &merged_rects {
                let rx = (r.x as u32).min(fb_w as u32);
                let ry = (r.y as u32).min(fb_h as u32);
                let rw = r.w.min((fb_w as u32).saturating_sub(rx));
                let rh = r.h.min((fb_h as u32).saturating_sub(ry));
                if rw > 0 && rh > 0 {
                    upload_rects.push((rx, ry, rw, rh));
                    total_size += (rw * rh * 4) as usize;
                }
            }

            // Prepare packed buffer
            self.window.scratch.resize(total_size, 0);
            let mut packed_offset = 0;

            for &(rx, ry, rw, rh) in &upload_rects {
                let rx = rx as usize;
                let ry = ry as usize;
                let rw = rw as usize;
                let rh = rh as usize;

                let dst_start = packed_offset;
                let mut di = dst_start;
                for row in ry..(ry + rh) {
                    let src_start = row * stride + rx * 4;
                    let row_bytes = rw * 4;

                    let dst_end = di + row_bytes;
                    if dst_end <= self.window.scratch.len()
                        && src_start + row_bytes <= framebuffer.len()
                    {
                        self.window.scratch[di..dst_end]
                            .copy_from_slice(&framebuffer[src_start..src_start + row_bytes]);
                        di += row_bytes;
                    }
                }
                packed_offset += rw * rh * 4;
            }
            upload_rects
        } else {
            vec![]
        };

        let mut overlays: Vec<crate::wgpu_render::OverlayParams> = Vec::new();

        let cursor_overlay = self.cursor.build_overlay();

        let mut text_sections: Vec<crate::wgpu_render::OwnedSection> = Vec::new();
        let phys = self.window.window.inner_size();
        let mut _ov_data: Vec<Vec<u8>> = Vec::new();
        let mut ov_descs: Vec<crate::wgpu_render::OverlayDesc> = Vec::new();

        if let Some(desc) = self
            .fps
            .build_overlay(phys.width, &mut text_sections, &mut _ov_data)
        {
            ov_descs.push(desc);
        }

        if let Some(desc) = self
            .pinbar
            .build(phys.width, &mut text_sections, &mut _ov_data)
        {
            ov_descs.push(desc);
        }

        for d in &ov_descs {
            overlays.push(crate::wgpu_render::OverlayParams {
                rgba: &_ov_data[d.data_idx],
                width: d.w,
                height: d.h,
                x: d.x,
                y: d.y,
                scale: d.scale,
            });
        }

        if !upload_rects.is_empty() {
            self.window.renderer.upload_gdi(
                &self.window.scratch,
                fb_w as u32,
                fb_h as u32,
                Some(&upload_rects),
            );
        }

        self.window.renderer.update_and_render(
            &[],
            fb_w as u32,
            fb_h as u32,
            &overlays,
            &text_sections,
            cursor_overlay.as_ref(),
            None,
        );

        Ok(())
    }

    pub fn request_screen_resize(&mut self) {
        if self.pendings.resize {
            return;
        }
        let phys = self.window.window.inner_size();
        let sf = self.coords_scale.max(1.0);
        let (rdp_w_raw, rdp_h_raw) =
            monitor::phys_2_logic((phys.width as i32, phys.height as i32), sf);
        let rdp_w = (rdp_w_raw as u32).max(1) & !3;
        let rdp_h = (rdp_h_raw as u32).max(1) & !3;

        log::info!(
            "request_screen_resize: phys={}x{} → rdp={rdp_w}x{rdp_h} (scale={sf})",
            phys.width,
            phys.height
        );

        self.window.renderer.reconfigure(phys.width, phys.height);
        self.last_resize = std::time::Instant::now();
        self.pendings.resize = true;

        if let Some(disp) = self.channels.write().unwrap().disp() {
            disp.send_monitor_layout(rdp_ffi::geom::Rect::new(0, 0, rdp_w, rdp_h), 0, 100, 100);
        }
    }

    pub fn on_desktop_resize(&mut self, _width: u32, _height: u32) {
        log::info!("DesktopResize acknowledged: {_width}x{_height}");
        self.pendings.resize = false;
        let phys = self.window.window.inner_size();
        self.window.renderer.reconfigure(phys.width, phys.height);
    }
}
