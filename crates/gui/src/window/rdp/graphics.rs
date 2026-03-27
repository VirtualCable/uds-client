// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
#![allow(dead_code)]
use std::sync::{atomic::Ordering, Arc, RwLock};

use shared::log;

use eframe::egui;

use rdp::sys::rdpGdi;

use super::connection::RdpConnectionState;
use crate::window::AppWindow;

#[derive(Clone)]
pub struct Screen {
    texture: egui::TextureId,
    texture_handle: egui::TextureHandle,
    size: egui::Vec2,
    use_rgba: bool,
    scratch: Vec<u8>,
}

impl Screen {
    pub fn new(ctx: &egui::Context, _frame: &mut eframe::Frame, size: egui::Vec2, use_rgba: bool) -> Self {
        let size_ux = [size.x as usize, size.y as usize];
        let image = egui::ColorImage::new(
            size_ux,
            vec![egui::Color32::TRANSPARENT; size_ux[0] * size_ux[1]],
        );
        let texture_handle = ctx.load_texture("rdp_screen", image, egui::TextureOptions::LINEAR);
        let texture_id = texture_handle.id();

        Self {
            texture: texture_id,
            texture_handle,
            size,
            use_rgba,
            scratch: Vec::with_capacity((size_ux[0] * size_ux[1] * 4).max(1024)),
        }
    }


    pub fn supports_bgra(_frame: &mut eframe::Frame) -> bool {
        #[cfg(target_os = "macos")]
        {
            false
        }
        #[cfg(not(target_os = "macos"))]
        {
            true
        }
    }

    pub fn update_screen_texture(
        &mut self,
        rects: &[rdp::geom::Rect],
        gdi: *mut rdpGdi,
        gdi_lock: &Arc<RwLock<()>>,
    ) {
        if rects.is_empty() {
            return;
        }

        let _gdi_guard = gdi_lock.read().unwrap();

        let (stride_bytes, fb_height, fb_width) = unsafe {
            (
                (*gdi).stride as usize,
                (*gdi).height as usize,
                (*gdi).width as usize,
            )
        };

        let framebuffer = unsafe {
            std::slice::from_raw_parts(
                (*gdi).primary_buffer as *const u8,
                stride_bytes * fb_height,
            )
        };

        let unique_rect = rects.iter().fold(None, |acc: Option<rdp::geom::Rect>, r| {
            if let Some(acc_rect) = acc {
                Some(acc_rect.union(r))
            } else {
                Some(*r)
            }
        });

        let rect = if let Some(r) = unique_rect { r } else { return };

        let safe_x = rect.x.min(fb_width as u32) as usize;
        let safe_y = rect.y.min(fb_height as u32) as usize;
        let safe_w = rect
            .w
            .min((fb_width as u32).saturating_sub(safe_x as u32));
        let safe_h = rect
            .h
            .min((fb_height as u32).saturating_sub(safe_y as u32));

        if safe_w == 0 || safe_h == 0 {
            return;
        }

        let needed = (safe_w * safe_h * 4) as usize;
        self.scratch.clear();
        self.scratch.reserve(needed);

        if self.use_rgba {
            for row in 0..safe_h {
                for col in 0..safe_w {
                    let px = safe_x + col as usize;
                    let py = safe_y + row as usize;
                    let idx = py * stride_bytes + px * 4;
                    self.scratch.push(framebuffer[idx]);
                    self.scratch.push(framebuffer[idx + 1]);
                    self.scratch.push(framebuffer[idx + 2]);
                    self.scratch.push(framebuffer[idx + 3]);
                }
            }
        } else {
            for row in 0..safe_h {
                for col in 0..safe_w {
                    let px = safe_x + col as usize;
                    let py = safe_y + row as usize;
                    let idx = py * stride_bytes + px * 4;
                    self.scratch.push(framebuffer[idx + 2]);
                    self.scratch.push(framebuffer[idx + 1]);
                    self.scratch.push(framebuffer[idx]);
                    self.scratch.push(framebuffer[idx + 3]);
                }
            }
        }

        let image = egui::ColorImage::from_rgba_premultiplied(
            [safe_w as usize, safe_h as usize],
            &self.scratch,
        );

        self.texture_handle.set_partial(
            [safe_x, safe_y],
            image,
            egui::TextureOptions::LINEAR,
        );
    }

    pub fn resize_screen_texture(&mut self, new_size: egui::Vec2) {
        if self.size == new_size {
            return;
        }

        self.size = new_size;
        let image = egui::ColorImage::new(
            [new_size.x as usize, new_size.y as usize],
            vec![egui::Color32::TRANSPARENT; (new_size.x as usize) * (new_size.y as usize)],
        );

        self.texture_handle
            .set(image, egui::TextureOptions::LINEAR);
    }

    pub fn texture_id(&self) -> egui::TextureId {
        self.texture
    }

    pub fn size(&self) -> egui::Vec2 {
        self.size
    }
}

impl AppWindow {
    pub(super) fn toggle_fullscreen(
        &mut self,
        ctx: &egui::Context,
        rdp_state: &mut RdpConnectionState,
    ) {
        log::debug!("ALT+ENTER pressed, toggling fullscreen");
        if rdp_state.full_screen.load(Ordering::Relaxed) {
            // Switch to fixed size, restores original size
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            rdp_state.full_screen.store(false, Ordering::Relaxed);
        } else {
            // Switch to fullscreen
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            rdp_state.full_screen.store(true, Ordering::Relaxed);
        }
    }

    pub(super) fn show_pinbar(&mut self, ui: &mut egui::Ui, rdp_state: &mut RdpConnectionState) {
        let fullscreen = rdp_state.full_screen.clone();
        if !rdp_state.pinbar_visible.load(Ordering::Relaxed) || !fullscreen.load(Ordering::Relaxed)
        {
            return;
        }

        egui::Area::new("pinbar".into())
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 0.0)) // Centered at top
            .order(egui::Order::Foreground) // Above all layers
            .constrain(true) // Keep within screen bounds
            .show(ui.ctx(), |ui| {
                // Frame with margins so it does not occupy the entire width
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin {
                        left: 64,
                        top: 8,
                        right: 16,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.label("UDS Connection");
                            ui.add_space(24.0);
                            ui.with_layout(
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    if ui.button("⬜").clicked() {
                                        self.toggle_fullscreen(ui.ctx(), rdp_state);
                                    }
                                    if ui.button("🗙").clicked() {
                                        self.exit(ui.ctx());
                                    }
                                },
                            );
                        });
                    });
            });
    }
}
