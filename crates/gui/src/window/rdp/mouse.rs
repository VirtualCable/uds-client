// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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
use std::sync::atomic::Ordering;

use eframe::egui;

use super::connection::RdpConnectionState;
use crate::window::AppWindow;

const FRAMES_IN_FLIGHT: usize = 128;

#[derive(Clone)]
pub struct RdpMouseCursor {
    pub texture: egui::TextureHandle,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl RdpMouseCursor {
    pub fn size_vec2(&self) -> egui::Vec2 {
        egui::Vec2::new(self.width as f32, self.height as f32)
    }

    pub fn position_pos2(&self) -> egui::Pos2 {
        egui::Pos2::new(self.x as f32, self.y as f32)
    }

    pub fn update(
        &mut self,
        texture: egui::TextureHandle,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) {
        self.texture = texture;
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
    }
}

impl AppWindow {
    pub(super) fn handle_cursor(&self, ctx: &egui::Context, rdp_state: &RdpConnectionState) {
        // Set custom cursor
        // Custom cursor, last to be on top
        if let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) {
            // If pointer is in bounds (2*width/5, 0) - (3*width/5, 2)
            let size = ctx.content_rect().size();
            if size.x * 2.0 / 5.0 < pos.x && pos.x < size.x * 3.0 / 5.0 && pos.y < 2.0 {
                // Also, show pinbar
                rdp_state.pinbar_visible.store(true, Ordering::Relaxed);
            } else if pos.y > 32.0 {
                // Hide pinbar if pointer is away
                rdp_state.pinbar_visible.store(false, Ordering::Relaxed);
            }

            // Default cursor for pinbar area
            if rdp_state.pinbar_visible.load(Ordering::Relaxed) {
                // If pinbar is visible, show default cursor
                ctx.set_cursor_icon(egui::CursorIcon::Default);
            } else {
                // Hide system cursor
                ctx.set_cursor_icon(egui::CursorIcon::None);
            }
            egui::Area::new("rdp_cursor_area".into())
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    self.show_custom_cursor(ui, &rdp_state.cursor.borrow(), pos);
                });
        }
    }

    fn show_custom_cursor(&self, ui: &mut egui::Ui, cursor: &RdpMouseCursor, pos: egui::Pos2) {
        // Add self.cursor texture at pos
        let cursor_size = cursor.size_vec2();
        let cursor_pos = egui::Pos2::new(pos.x - cursor.x as f32, pos.y - cursor.y as f32);
        ui.painter().image(
            cursor.texture.id(),
            egui::Rect::from_min_size(cursor_pos, cursor_size),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }

    pub(super) fn set_custom_cursor(
        &self,
        ctx: &egui::Context,
        rdp_state: &mut RdpConnectionState,
        cursor_data: &[u8],
        rect: rdp::geom::Rect,
    ) {
        let cursor_image = egui::ColorImage::from_rgba_unmultiplied(
            [rect.w as usize, rect.h as usize],
            cursor_data,
        );
        rdp_state.cursor.borrow_mut().update(
            ctx.load_texture("rdp_cursor", cursor_image, egui::TextureOptions::LINEAR),
            rect.x,
            rect.y,
            rect.w,
            rect.h,
        );
    }
}
