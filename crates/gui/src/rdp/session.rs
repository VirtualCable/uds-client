use anyhow::Result;
use shared::log;

use super::RdpState;
use crate::monitor;

impl RdpState {
    pub fn update_screen(&mut self) -> Result<()> {
        let gdi = self.gdi;

        let (stride, fb_h, fb_w) = {
            let _lock = self.gdi_lock.read().unwrap();
            unsafe {
                (
                    (*gdi).stride as usize,
                    (*gdi).height as usize,
                    (*gdi).width as usize,
                )
            }
        };

        if fb_w == 0 || fb_h == 0 {
            log::warn!("update_screen: GDI dimensions are 0, skipping");
            return Ok(());
        }

        let need_swizzle = !cfg!(target_os = "macos");
        let total = fb_w * fb_h * 4;
        self.window.scratch.resize(total, 0);

        {
            let _lock = self.gdi_lock.read().unwrap();
            let framebuffer = unsafe {
                std::slice::from_raw_parts((*gdi).primary_buffer as *const u8, stride * fb_h)
            };

            for row in 0..fb_h {
                let src_start = row * stride;
                let dst_start = row * fb_w * 4;
                let row_bytes = (fb_w * 4).min(framebuffer.len().saturating_sub(src_start));
                let dst_end = (dst_start + row_bytes).min(self.window.scratch.len());
                if need_swizzle {
                    for col in 0..fb_w {
                        let si = src_start + col * 4;
                        let di = dst_start + col * 4;
                        if si + 3 < framebuffer.len() && di + 3 < self.window.scratch.len() {
                            self.window.scratch[di] = framebuffer[si + 2];
                            self.window.scratch[di + 1] = framebuffer[si + 1];
                            self.window.scratch[di + 2] = framebuffer[si];
                            self.window.scratch[di + 3] = framebuffer[si + 3];
                        }
                    }
                } else {
                    self.window.scratch[dst_start..dst_end]
                        .copy_from_slice(&framebuffer[src_start..src_start + row_bytes]);
                }
            }
        }

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

        self.window.renderer.update_and_render(
            &self.window.scratch,
            fb_w as u32,
            fb_h as u32,
            &overlays,
            &text_sections,
            cursor_overlay.as_ref(),
        );

        Ok(())
    }

    pub fn request_screen_resize(&mut self) {
        if self.last_resize.elapsed().as_millis() < 500 {
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
        self.pending_resize = true;

        if let Some(disp) = self.channels.write().unwrap().disp() {
            disp.send_monitor_layout(rdp_ffi::geom::Rect::new(0, 0, rdp_w, rdp_h), 0, 100, 100);
        }
    }

    pub fn on_desktop_resize(&mut self, _width: u32, _height: u32) {
        log::info!("DesktopResize acknowledged: {_width}x{_height}");
        self.pending_resize = false;
        let phys = self.window.window.inner_size();
        self.window.renderer.reconfigure(phys.width, phys.height);
    }
}
