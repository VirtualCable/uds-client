#![allow(dead_code)]
use std::{
    fmt,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::Result;
use crossbeam::channel::{Receiver, Sender, bounded};
use eframe::{
    egui,
    glow::{self, HasContext, PixelUnpackData},
};

use crate::{log, logo::load_logo};

use rdp::{
    Rdp,
    messaging::RdpMessage,
    settings::RdpSettings,
    sys::{rdpGdi, rdpInput},
};

use super::{
    AppWindow,
    types::{AppState, HotKey},
};

const FRAMES_IN_FLIGHT: usize = 128;

#[derive(Clone)]
struct RdpMouseCursor {
    texture: egui::TextureHandle,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl RdpMouseCursor {
    fn size_vec2(&self) -> egui::Vec2 {
        egui::Vec2::new(self.width as f32, self.height as f32)
    }

    fn position_pos2(&self) -> egui::Pos2 {
        egui::Pos2::new(self.x as f32, self.y as f32)
    }

    fn update(&mut self, texture: egui::TextureHandle, x: u32, y: u32, width: u32, height: u32) {
        self.texture = texture;
        self.x = x;
        self.y = y;
        self.width = width;
        self.height = height;
    }
}

// Arcs are to keep original references when cloning
// because states are cloned when switching app states
#[derive(Clone)]
pub struct RdpConnectionState {
    update_rx: crossbeam::channel::Receiver<RdpMessage>,
    gdi: *mut rdpGdi,
    gdi_lock: Arc<RwLock<()>>,
    input: *mut rdpInput,
    channels: Arc<RwLock<rdp::channels::RdpChannels>>,
    texture: egui::TextureId,
    native_texture: glow::Texture,
    texture_size: egui::Vec2,
    cursor: Arc<RwLock<RdpMouseCursor>>,
    full_screen: Arc<AtomicBool>,
    // For top pinbar
    pinbar_visible: Arc<AtomicBool>,
}

impl fmt::Debug for RdpConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RdpState")
            .field("gdi", &self.gdi)
            .field("input", &self.input)
            .finish()
    }
}

impl AppWindow {
    pub fn enter_rdp_connection(
        &mut self,
        ctx: &eframe::egui::Context,
        frame: &mut eframe::Frame,
        rdp_settings: RdpSettings,
    ) -> Result<()> {
        self.processing_events.store(true, Ordering::Relaxed); // Start processing events
        let (tx, rx): (Sender<RdpMessage>, Receiver<RdpMessage>) = bounded(FRAMES_IN_FLIGHT);

        let mut rdp_settings = rdp_settings;
        // TODO: Handle screen size changes during session with RDP display channel
        let is_full_screen = if rdp_settings.screen_size.is_fullscreen() {
            let real_size = ctx.content_rect().size();
            rdp_settings.screen_size =
                rdp::geom::ScreenSize::Fixed(real_size.x as u32, real_size.y as u32);
            true
        } else {
            false
        };

        // Rdp shouls be pinned, as build() inserts self reference inside freedrp structs
        let mut rdp = Box::pin(Rdp::new(rdp_settings, tx));

        // For reference: Currently, default callbacks are these also, so if no more are needed, this can be skipped
        // rdp.set_update_callbacks(vec![
        //     update_c::Callbacks::BeginPaint,
        //     update_c::Callbacks::EndPaint,
        //     update_c::Callbacks::DesktopResize,
        // ]);
        rdp.as_mut().build()?; // Build inserts "rdp" inside an struct for freedrp, must ensure that rdp does not move after this point

        log::debug!("** Rdp address: {:p}", &rdp);

        rdp.connect()?;

        #[cfg(debug_assertions)]
        {
            rdp.debug_assert_instance();
        }

        let rdpversion_str = rdp.get_rdp_version()?;

        log::debug!("Connected. RDP Version: {}", rdpversion_str);

        // Ge the gdi pointer
        let gdi = rdp
            .gdi()
            .ok_or_else(|| anyhow::anyhow!("GDI not initialized"))?;
        let input = rdp
            .input()
            .ok_or_else(|| anyhow::anyhow!("Input not initialized"))?;
        // And the lock
        let gdi_lock = rdp.gdi_lock();

        log::debug!("Obtained GDI pointer: {:?}", gdi);
        log::debug!("Gdi: {:?}", unsafe { *gdi });

        let texture_size = egui::Vec2::new(unsafe { (*gdi).width as f32 }, unsafe {
            (*gdi).height as f32
        });

        let (texture, native_texture) =
            self.new_texture(frame, texture_size.x as u32, texture_size.y as u32);

        let cursor_img = load_logo();
        let cursor_img_size = cursor_img.size;
        let cursor = ctx.load_texture("rdp_cursor", cursor_img, egui::TextureOptions::LINEAR);

        self.set_app_state(AppState::RdpConnected(RdpConnectionState {
            update_rx: rx,
            gdi,
            input,
            channels: rdp.channels().clone(),
            gdi_lock,
            texture,
            native_texture,
            texture_size,
            cursor: Arc::new(RwLock::new(RdpMouseCursor {
                texture: cursor,
                x: 0,
                y: 0,
                width: cursor_img_size[0] as u32,
                height: cursor_img_size[1] as u32,
            })),
            full_screen: Arc::new(AtomicBool::new(is_full_screen)),
            pinbar_visible: Arc::new(AtomicBool::new(false)),
        }));

        std::thread::spawn(move || {
            // Note: This may already be marked as launched from external RDP launcher
            // But ensure it is marked here as well (to allow using from other gui launchers as test app)
            shared::tasks::mark_internal_rdp_as_running();
            let res = rdp.run();
            shared::tasks::mark_internal_rdp_as_not_running();
            log::debug!("RDP thread exiting...");
            if let Err(e) = res {
                log::debug!("RDP thread ended with error: {}", e);
            } else {
                log::debug!("RDP thread ended.");
            }
        });
        self.processing_events.store(true, Ordering::Relaxed); // Start processing events

        Ok(())
    }

    pub(super) fn update_rdp_connection(
        &mut self,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
        mut rdp_state: RdpConnectionState,
    ) {
        // Calculate relation between gdi size and egui content size
        let scale = {
            let egui_size = ctx.content_rect().size();
            let gdi_width = unsafe { (*rdp_state.gdi).width as f32 };
            let gdi_height = unsafe { (*rdp_state.gdi).height as f32 };
            egui::Vec2::new(gdi_width / egui_size.x, gdi_height / egui_size.y)
        };

        if self.handle_hotkeys(ctx, &mut rdp_state) {
            // Hotkey handled, skip input processing this frame
            return;
        }
        let input = rdp_state.input;
        self.handle_input(ctx, input, scale);

        let gl = frame.gl().unwrap();
        //     y: actual_height,
        // } = ctx.content_rect().size();
        // let (actual_width, actual_height) = (actual_width as i32, actual_height as i32);
        // let (gdi_width, gdi_height) = unsafe { ((*rdp_state.gdi).width, (*rdp_state.gdi).height) };

        // if actual_width != gdi_width || actual_height != gdi_height {
        //     log::debug!(
        //         "Viewport size changed: actual=({}, {}), gdi=({}, {}), resizing gdi and texture",
        //         actual_width,
        //         actual_height,
        //         gdi_width,
        //         gdi_height
        //     );
        // }
        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(0.0))
            .show(ctx, |ui| {
                // If the size of gdi is not equal to size of content, resize gdi and recreate texture
                let start = std::time::Instant::now();
                let mut rects_to_update: Vec<rdp::geom::Rect> = Vec::new();
                while let Ok(message) = rdp_state.update_rx.try_recv() {
                    log::trace!("Got message {:?}", message);
                    // Process all pending messages BUT only the last update_rect to avoid lagging behind
                    match message {
                        RdpMessage::UpdateRects(rects) => {
                            // TODO: Implement several optimizations:
                            // 1.- The textue updater shoud be a thread with a channel, so we just send the rects to update here
                            // 2.- The texture updater should merge rects to minimize updates with all the rects it has pending
                            rects_to_update.extend_from_slice(&rects);
                        }
                        RdpMessage::Disconnect => {
                            log::debug!("RDP Disconnected");
                            self.exit(ctx);
                            break;
                        }
                        RdpMessage::Error(err) => {
                            log::error!("RDP Error: {}", err);
                            self.exit(ctx);
                            break;
                        }
                        RdpMessage::FocusRequired => {
                            log::debug!("RDP Focus Required");
                        }
                        RdpMessage::SetCursorIcon(data, x, y, width, height) => {
                            // log::debug!("Setting cursor icon, size: {width}x{height} on {x}, {y}");
                            let cursor_image = egui::ColorImage::from_rgba_unmultiplied(
                                [width as usize, height as usize],
                                &data,
                            );
                            rdp_state.cursor.write().unwrap().update(
                                ctx.load_texture(
                                    "rdp_cursor",
                                    cursor_image,
                                    egui::TextureOptions::LINEAR,
                                ),
                                x,
                                y,
                                width,
                                height,
                            );
                        }
                    }
                }
                Self::update_texture(gl, rects_to_update, rdp_state.clone());
                log::trace!("RDP update processing took {:?}", start.elapsed());
                // Show the texture on 0,0, full size
                let size = ui.available_size();
                ui.add_sized(
                    size,
                    egui::Image::new(egui::load::SizedTexture::new(
                        rdp_state.texture,
                        rdp_state.texture_size,
                    )),
                );

                log::trace!("RDP frame rendered took {:?}", start.elapsed());
            });
        // Pinbar at top
        self.show_pinbar(ctx, &mut rdp_state);
        // Handle custom cursor
        self.handle_cursor(ctx, &rdp_state);
    }

    fn handle_hotkeys(&mut self, ctx: &egui::Context, rdp_state: &mut RdpConnectionState) -> bool {
        match HotKey::from_input(ctx) {
            HotKey::ToggleFullScreen => {
                self.toggle_fullscreen(ctx, rdp_state);
                true
            }
            HotKey::None => false,
        }
    }

    fn toggle_fullscreen(&mut self, ctx: &egui::Context, rdp_state: &mut RdpConnectionState) {
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

    fn new_texture(
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
    fn update_texture(
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
                gl.bind_texture(glow::TEXTURE_2D, Some(rdp_state.native_texture));

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

    fn handle_cursor(&self, ctx: &egui::Context, rdp_state: &RdpConnectionState) {
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
                    self.custom_cursor(ui, &rdp_state.cursor.read().unwrap(), pos);
                });
        }
    }

    fn custom_cursor(&self, ui: &mut egui::Ui, cursor: &RdpMouseCursor, pos: egui::Pos2) {
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

    fn show_pinbar(&mut self, ctx: &egui::Context, rdp_state: &mut RdpConnectionState) {
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
