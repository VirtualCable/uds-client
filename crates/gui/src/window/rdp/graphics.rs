#![allow(dead_code)]
use std::sync::atomic::Ordering;

use shared::log;

use eframe::{
    egui,
    glow::{self, HasContext, PixelUnpackData},
};

use super::rdp_connection::RdpConnectionState;
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

#[derive(Clone, Debug)]
pub struct Screen {
    texture: egui::TextureId,
    native_texture: glow::Texture,
    size: egui::Vec2,
}

impl Screen {
    pub fn new(texture: egui::TextureId, native_texture: glow::Texture, size: egui::Vec2) -> Self {
        Self {
            texture,
            native_texture,
            size,
        }
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

    pub(super) fn new_screen_texture(
        &self,
        frame: &mut eframe::Frame,
        width: u32,
        height: u32,
    ) -> (egui::TextureId, glow::Texture) {
        let gl = frame.gl().unwrap();
        let native_texture = unsafe { gl.create_texture().unwrap() };
        unsafe {
            gl.bind_texture(glow::TEXTURE_2D, Some(native_texture));
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::BGRA as i32,
                width as i32,
                height as i32,
                0,
                glow::BGRA,
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
        (
            frame.register_native_glow_texture(native_texture),
            native_texture,
        )
    }

    /// Update only the changed rects in a separate thread
    pub(super) fn update_screen_texture(
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

        let screen = rdp_state.screen.clone();

        if let Some(rect) = unique_rect {
            unsafe {
                gl.bind_texture(glow::TEXTURE_2D, Some(screen.native()));

                // Configurar cómo se interpreta el buffer
                gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, stride_pixels as i32);
                gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, rect.x as i32);
                gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, rect.y as i32);

                // Copiar subimagen directamente (BGRA nativo)
                gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0, // mip level
                    rect.x as i32,
                    rect.y as i32,
                    rect.w as i32,
                    rect.h as i32,
                    glow::BGRA, // formato de origen
                    glow::UNSIGNED_BYTE,
                    PixelUnpackData::Slice(Some(framebuffer)),
                );

                // Restaurar estado para no afectar otras cargas
                gl.pixel_store_i32(glow::UNPACK_ROW_LENGTH, 0);
                gl.pixel_store_i32(glow::UNPACK_SKIP_PIXELS, 0);
                gl.pixel_store_i32(glow::UNPACK_SKIP_ROWS, 0);
            }
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
