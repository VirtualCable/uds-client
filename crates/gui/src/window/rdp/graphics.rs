#![allow(dead_code)]
use std::sync::atomic::Ordering;

use shared::log;

use eframe::{
    egui,
    glow::{self, HasContext, PixelUnpackData},
};

use super::connection::RdpConnectionState;
use crate::window::AppWindow;

#[derive(Clone, Debug)]
pub struct Screen {
    texture: egui::TextureId,
    native_texture: glow::Texture,
    size: egui::Vec2,
    use_rgba: bool,
}

impl Screen {
    pub fn new(frame: &mut eframe::Frame, size: egui::Vec2, use_rgba: bool) -> Self {
        let gl = frame.gl().unwrap();
        let native_texture = unsafe { gl.create_texture().unwrap() };

        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(native_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                if use_rgba { glow::RGBA } else { glow::BGRA } as i32,
                size.x as i32,
                size.y as i32,
                0,
                if use_rgba { glow::RGBA } else { glow::BGRA },
                glow::UNSIGNED_BYTE,
                PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
        }

        // TextureId is for egui to identify the texture
        // native is for our operations
        Screen {
            texture: frame.register_native_glow_texture(native_texture),
            native_texture,
            size,
            use_rgba,
        }
    }

    pub fn supports_bgra(_frame: &mut eframe::Frame) -> bool {
        // All except macOS support BGRA natively
        #[cfg(target_os = "macos")]
        {
            false
        }
        #[cfg(not(target_os = "macos"))]
        {
            true
        }
    }

    /// Update only the changed rects in a separate thread
    pub fn update_screen_texture(
        &self,
        gl: &glow::Context,
        rects: Vec<rdp::geom::Rect>,
        rdp_state: RdpConnectionState,
    ) {
        // If already updating, or no rects, skip
        if rects.is_empty() {
            return;
        }
        let (stride_bytes, fb_height) = unsafe {
            (
                (*rdp_state.gdi).stride as usize,
                (*rdp_state.gdi).height as usize,
            )
        };
        let framebuffer = unsafe {
            std::slice::from_raw_parts(
                (*rdp_state.gdi).primary_buffer as *const u8,
                stride_bytes * fb_height,
            )
        };
        let stride_pixels = stride_bytes / 4;

        // Fold all rects into one union rect to minimize updates
        let unique_rect = rects.iter().fold(None, |acc: Option<rdp::geom::Rect>, r| {
            if let Some(acc_rect) = acc {
                Some(acc_rect.union(r))
            } else {
                Some(*r)
            }
        });

        if let Some(rect) = unique_rect {
            unsafe {
                gl.bind_texture(glow::TEXTURE_2D, Some(self.native()));

                gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, stride_pixels as i32);
                gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, rect.x as i32);
                gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, rect.y as i32);

                // Upload only the changed rect using texSubImage2D and desired format
                gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0, // mip level
                    rect.x as i32,
                    rect.y as i32,
                    rect.w as i32,
                    rect.h as i32,
                    if self.use_rgba {
                        glow::RGBA
                    } else {
                        glow::BGRA
                    }, // source format
                    glow::UNSIGNED_BYTE,
                    PixelUnpackData::Slice(Some(framebuffer)),
                );

                // Reset pixel store parameters
                gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
                gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
                gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);
            }
        }
    }

    pub fn resize_screen_texture(&mut self, gl: &glow::Context, new_size: egui::Vec2) {
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(self.native_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                if self.use_rgba {
                    glow::RGBA
                } else {
                    glow::BGRA
                } as i32,
                new_size.x as i32,
                new_size.y as i32,
                0,
                if self.use_rgba {
                    glow::RGBA
                } else {
                    glow::BGRA
                },
                glow::UNSIGNED_BYTE,
                PixelUnpackData::Slice(None), // Empty content
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
        }

        // Update the stored size
        self.size = new_size;
    }

    pub fn texture_id(&self) -> egui::TextureId {
        self.texture
    }

    pub fn native(&self) -> glow::Texture {
        self.native_texture
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
        log::debug!("Channels: {:?}", rdp_state.channels.read().unwrap());
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

    pub(super) fn show_pinbar(&mut self, ctx: &egui::Context, rdp_state: &mut RdpConnectionState) {
        let fullscreen = rdp_state.full_screen.clone();
        if !rdp_state.pinbar_visible.load(Ordering::Relaxed) || !fullscreen.load(Ordering::Relaxed)
        {
            return;
        }

        egui::Area::new("pinbar".into())
            .fixed_pos(egui::pos2(0.0, 0.0)) // Esquina superior izquierda
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 0.0)) // Centrado arriba
            .order(egui::Order::Foreground) // Encima de todo
            .constrain(true) // Mantener dentro de pantalla
            .show(ctx, |ui| {
                // Frame con márgenes para no ocupar todo el ancho
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
                                        self.toggle_fullscreen(ctx, rdp_state);
                                    }
                                    if ui.button("🗙").clicked() {
                                        self.exit(ctx);
                                    }
                                },
                            );
                        });
                    });
            });
    }
}
