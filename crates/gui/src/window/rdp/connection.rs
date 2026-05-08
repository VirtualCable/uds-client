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
use std::{
    cell::RefCell,
    fmt,
    rc::Rc,
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::Result;
use eframe::egui;
use flume::{Receiver, Sender, bounded};

use crate::{log, logo::load_logo};

use rdp::{
    Rdp,
    messaging::RdpMessage,
    settings::RdpSettings,
    sys::{rdpGdi, rdpInput},
};

use crate::window::{AppWindow, types::AppState};

const FRAMES_IN_FLIGHT: usize = 128;

#[derive(Clone)]
pub struct RemoteWindow {
    pub id: u32,
    pub title: String,
    pub rect: rdp::geom::Rect,
    pub show_state: Option<u32>,
    pub is_offscreen: bool,
    pub texture: Option<egui::TextureHandle>,
}

#[derive(Clone, Copy)]
pub struct SafeInputPtr(pub *mut rdpInput);
unsafe impl Send for SafeInputPtr {}
unsafe impl Sync for SafeInputPtr {}

impl fmt::Debug for SafeInputPtr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SafeInputPtr").field(&self.0).finish()
    }
}

// Arcs are to keep original references when cloning
// because states are cloned when switching app states
#[derive(Clone)]
pub struct RdpConnectionState {
    pub update_rx: Receiver<RdpMessage>,
    pub gdi: *mut rdpGdi,
    pub gdi_lock: Arc<RwLock<()>>,
    pub input: SafeInputPtr,
    pub channels: Arc<RwLock<rdp::channels::RdpChannels>>,
    pub screen: super::graphics::Screen,
    pub cursor: Rc<RefCell<super::mouse::RdpMouseCursor>>,
    pub full_screen: Rc<AtomicBool>,
    // For top pinbar
    pub pinbar_visible: Rc<AtomicBool>,

    // For resize, to avoiid too fast resizes
    pub last_resize: Rc<RefCell<std::time::Instant>>,

    // FPS
    // Very basic fps calculation
    // We get time between frames
    pub fps: Rc<RefCell<super::fps::Fps>>,

    // RAIL / RemoteApp mode
    pub is_rail: bool,
    pub remote_windows: Rc<RefCell<std::collections::HashMap<u32, RemoteWindow>>>,
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

        let is_full_screen = if rdp_settings.screen_size.is_fullscreen() {
            let real_size = ctx.content_rect().size();
            rdp_settings.screen_size =
                rdp::geom::ScreenSize::Fixed(real_size.x as u32, real_size.y as u32);
            true
        } else {
            false
        };

        let use_rgba = !super::graphics::Screen::supports_bgra(frame);

        let is_rail = rdp_settings.rail_app.is_some();

        if is_rail {
            // Instead of hiding the main window (which pauses the event loop and stops updates),
            // we make it small and show a status message.
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize([300.0, 100.0].into()));
            ctx.send_viewport_cmd(egui::ViewportCommand::Title("UDS RemoteApp".to_owned()));
        }

        let mut rdp_instance = Rdp::new(rdp_settings, tx, use_rgba);

        if is_rail {
            rdp_instance.set_window_callbacks(vec![
                rdp::callbacks::window_c::Callbacks::Create,
                rdp::callbacks::window_c::Callbacks::Update,
                rdp::callbacks::window_c::Callbacks::Delete,
            ]);
        }

        // Rdp shouls be pinned, as build() inserts self reference inside freedrp structs
        let mut rdp: std::pin::Pin<Box<Rdp>> = Box::pin(rdp_instance);

        // For reference: Currently, default callbacks are these also, so if no more are needed, this can be skipped
        // rdp.set_update_callbacks(vec![
        //     update_c::Callbacks::BeginPaint,
        //     update_c::Callbacks::EndPaint,
        //     update_c::Callbacks::DesktopResize,
        // ]);
        rdp.as_mut().build()?; // Build inserts "rdp" inside an struct for freedrp, must ensure that rdp does not move after this point

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

        let texture_size = egui::Vec2::new(unsafe { (*gdi).width as f32 }, unsafe {
            (*gdi).height as f32
        });

        let cursor_img = load_logo();
        let cursor_img_size = cursor_img.size;
        let cursor = ctx.load_texture("rdp_cursor", cursor_img, egui::TextureOptions::LINEAR);

        self.set_app_state(AppState::RdpConnected(RdpConnectionState {
            update_rx: rx,
            gdi,
            input: SafeInputPtr(input),
            channels: rdp.channels().clone(),
            gdi_lock,
            screen: super::graphics::Screen::new(ctx, frame, texture_size, use_rgba),
            cursor: Rc::new(RefCell::new(super::mouse::RdpMouseCursor {
                texture: cursor,
                x: 0,
                y: 0,
                width: cursor_img_size[0] as u32,
                height: cursor_img_size[1] as u32,
            })),
            full_screen: Rc::new(AtomicBool::new(is_full_screen)),
            pinbar_visible: Rc::new(AtomicBool::new(false)),
            last_resize: Rc::new(RefCell::new(std::time::Instant::now())),
            fps: Rc::new(RefCell::new(super::fps::Fps::new())),
            is_rail,
            remote_windows: Rc::new(RefCell::new(std::collections::HashMap::new())),
        }));

        std::thread::spawn(move || {
            // Note: This may already be marked as launched from external RDP launcher
            // But ensure it is marked here as well (to allow using from other gui launchers as test app)
            connection::tasks::mark_internal_rdp_as_running();
            let res = rdp.run();
            connection::tasks::mark_internal_rdp_as_not_running();
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

    pub fn update_rdp_connection(
        &mut self,
        ui: &mut egui::Ui,
        _frame: &mut eframe::Frame,
        mut rdp_state: RdpConnectionState,
    ) {
        // Calculate relation between gdi size and egui content size
        let scale = {
            let egui_size = ui.ctx().content_rect().size();
            let gdi_width = unsafe { (*rdp_state.gdi).width as f32 };
            let gdi_height = unsafe { (*rdp_state.gdi).height as f32 };
            egui::Vec2::new(gdi_width / egui_size.x, gdi_height / egui_size.y)
        };
        if self.handle_hotkeys(ui.ctx(), &mut rdp_state) {
            // Hotkey handled, skip input processing this frame
            return;
        }

        let input = rdp_state.input.0;
        if !rdp_state.is_rail {
            self.handle_input(ui.ctx(), input, scale);
            self.handle_screen_resize(ui.ctx().content_rect().size(), &mut rdp_state);
        } else {
            ui.ctx().input(|i| {
                self.handle_keyboard(ui.ctx(), input, i);
            });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(0.0))
            .show_inside(ui, |ui| {
                // If the size of gdi is not equal to size of content, resize gdi and recreate texture
                // let start = std::time::Instant::now();
                let mut rects_to_update: Vec<rdp::geom::Rect> = Vec::new();
                while let Ok(message) = rdp_state.update_rx.try_recv() {
                    log::trace!("Got message {:?}", message);
                    // Process all pending messages BUT only the last update_rect to avoid lagging behind
                    match message {
                        RdpMessage::UpdateRects(rects) => {
                            log::info!("GUI received UpdateRects: count={}", rects.len());
                            rects_to_update.extend_from_slice(&rects);
                        }
                        RdpMessage::WindowCreate {
                            window_id,
                            title,
                            show_state,
                            is_offscreen,
                            pos,
                            size,
                        } => {
                            log::info!("GUI received WindowCreate: id={}, title={}, pos={:?}, size={:?}", window_id, title, pos, size);
                            let mut windows = rdp_state.remote_windows.borrow_mut();
                            let existing_rect = windows.get(&window_id).map(|w| w.rect).unwrap_or(rdp::geom::Rect::new(0, 0, 0, 0));
                            
                            let mut new_rect = existing_rect;
                            if let Some((x, y)) = pos {
                                new_rect.x = x;
                                new_rect.y = y;
                            }
                            if let Some((w, h)) = size {
                                new_rect.w = w;
                                new_rect.h = h;
                            }

                            windows.insert(
                                window_id,
                                RemoteWindow {
                                    id: window_id,
                                    title,
                                    rect: new_rect,
                                    show_state,
                                    is_offscreen: is_offscreen.unwrap_or(false),
                                    texture: None,
                                },
                            );
                        }
                        RdpMessage::WindowUpdate {
                            window_id,
                            title,
                            show_state,
                            is_offscreen,
                            pos,
                            size,
                        } => {
                            log::info!("GUI received WindowUpdate: id={}, title={}, pos={:?}, size={:?}", window_id, title, pos, size);
                            let mut windows = rdp_state.remote_windows.borrow_mut();
                            if let Some(w) = windows.get_mut(&window_id) {
                                if !title.is_empty() {
                                    w.title = title;
                                }
                                if let Some(s) = show_state {
                                    w.show_state = Some(s);
                                }
                                if let Some(o) = is_offscreen {
                                    w.is_offscreen = o;
                                }
                                if let Some((x, y)) = pos {
                                    w.rect.x = x;
                                    w.rect.y = y;
                                }
                                if let Some((width, height)) = size {
                                    w.rect.w = width;
                                    w.rect.h = height;
                                }
                            }
                        }
                        RdpMessage::WindowDelete(window_id) => {
                            log::info!("GUI received WindowDelete: id={}", window_id);
                            rdp_state.remote_windows.borrow_mut().remove(&window_id);
                        }
                        RdpMessage::ClientWindowMove { .. } => {}
                        RdpMessage::ClientSystemCommand { .. } => {}
                        RdpMessage::Disconnect => {
                            log::debug!("RDP Disconnected");
                            self.exit(ui.ctx());
                            break;
                        }
                        RdpMessage::Error(err) => {
                            log::error!("RDP Error: {}", err);
                            self.exit(ui.ctx());
                            break;
                        }
                        RdpMessage::FocusRequired => {
                            log::debug!("RDP Focus Required");
                        }
                        RdpMessage::SetCursorIcon(data, x, y, width, height) => {
                            // log::debug!("Setting cursor icon, size: {width}x{height} on {x}, {y}");
                            self.set_custom_cursor(
                                ui.ctx(),
                                &mut rdp_state,
                                &data,
                                rdp::geom::Rect {
                                    x: x as i32,
                                    y: y as i32,
                                    w: width,
                                    h: height,
                                },
                            );
                        }
                        RdpMessage::WindowPixels {
                            window_id,
                            width,
                            height,
                            data,
                        } => {
                            log::debug!("GUI received WindowPixels: id={}, size={}x{}", window_id, width, height);
                            let mut windows = rdp_state.remote_windows.borrow_mut();
                            if let Some(w) = windows.get_mut(&window_id) {
                                let mut data_with_alpha = data;
                                // Force alpha to 255 just in case
                                for chunk in data_with_alpha.chunks_exact_mut(4) {
                                    if chunk[3] == 0 {
                                        chunk[3] = 255;
                                    }
                                }
                                let image = egui::ColorImage::from_rgba_unmultiplied(
                                    [width as usize, height as usize],
                                    &data_with_alpha,
                                );
                                if let Some(tex) = &mut w.texture {
                                    tex.set(image, egui::TextureOptions::LINEAR);
                                } else {
                                    w.texture = Some(ui.ctx().load_texture(
                                        format!("window_{}", window_id),
                                        image,
                                        egui::TextureOptions::LINEAR,
                                    ));
                                }
                            }
                        }
                        RdpMessage::ClipboardData(_) => {}
                        RdpMessage::MicConfig { .. } => {}
                        RdpMessage::None => {}
                    }
                }
                // log::debug!("RDP message processing took {:?} with {} rects", start.elapsed(), rects_to_update.len());
                rdp_state.screen.update_screen_texture(
                    &rects_to_update,
                    rdp_state.gdi,
                    &rdp_state.gdi_lock,
                );

                if rdp_state.is_rail {
                    // Draw main window UI for RAIL mode
                    ui.centered_and_justified(|ui| {
                        ui.heading("UDS RemoteApp Connection Active");
                        ui.add_space(10.0);
                        if ui.button("Disconnect").clicked() {
                            self.exit(ui.ctx());
                        }
                    });

                    // Draw the remote windows as viewports
                    let windows = rdp_state.remote_windows.borrow().clone();
                    let rail_channel = rdp_state.channels.read().unwrap().rail();
                    let safe_input = rdp_state.input;
                    
                    for (_, window) in windows {
                        if window.is_offscreen || window.rect.w == 0 || window.rect.h == 0 {
                            continue;
                        }
                        
                        if window.show_state == Some(0) { // 0 = SW_HIDE
                            continue;
                        }

                        let texture_id = if let Some(tex) = &window.texture {
                            tex.id()
                        } else {
                            // If we don't have window pixels yet, skip drawing this viewport to avoid black flashes
                            continue;
                        };
                        
                        let id = egui::ViewportId::from_hash_of(window.id);
                        
                        let rect = window.rect;
                        let offset = egui::Vec2::new(rect.x as f32, rect.y as f32);
                        
                        // We use the whole window texture now, not a mapped UV of the primary desktop!
                        let uv = egui::Rect::from_min_max(
                            egui::pos2(0.0, 0.0),
                            egui::pos2(1.0, 1.0)
                        );
                        
                        let rail_channel_clone = rail_channel.clone();
                        let window_id = window.id;

                        ui.ctx().show_viewport_deferred(
                            id,
                            egui::ViewportBuilder::default()
                                .with_title(&window.title)
                                .with_inner_size([rect.w as f32, rect.h as f32])
                                .with_position(egui::pos2(rect.x as f32, rect.y as f32))
                                .with_decorations(true)
                                .with_transparent(false)
                                .with_visible(true),
                            move |ctx, _class| {
                                let safe_input = safe_input; // Force capture the wrapper, not just the raw ptr
                                egui::CentralPanel::default()
                                    .frame(egui::Frame::default().inner_margin(0.0))
                                    .show_inside(ctx, |ui| {
                                        ui.add_sized(
                                            [rect.w as f32, rect.h as f32],
                                            egui::Image::new(egui::load::SizedTexture::new(
                                                texture_id,
                                                [rect.w as f32, rect.h as f32]
                                            )).uv(uv)
                                        );
                                    });
                                
                                ctx.input(|i| {
                                    AppWindow::handle_mouse(ctx, safe_input.0, i, egui::Vec2::new(1.0, 1.0), offset);
                                });

                                if ctx.input(|i| i.viewport().close_requested()) {
                                    if let Some(rail) = &rail_channel_clone {
                                        rail.send_system_command(window_id, rdp::consts::SC_CLOSE as u16);
                                    }
                                }
                            }
                        );
                    }
                } else {
                    // Show the texture on 0,0, full size
                    let size = ui.available_size();
                    ui.add_sized(
                        size,
                        egui::Image::new(egui::load::SizedTexture::new(
                            rdp_state.screen.texture_id(),
                            size,
                        )),
                    );
                }

                //log::debug!("RDP frame rendered took {:?}", start.elapsed());
            });
        // Pinbar at top
        self.show_pinbar(ui, &mut rdp_state);

        rdp_state.fps.borrow_mut().record_frame();
        // Handle custom cursor
        self.handle_cursor(ui, &rdp_state);

        // Fps if enabled, last so it goes on top
        rdp_state.fps.borrow().show(ui.ctx());
    }

    fn handle_screen_resize(
        &mut self,
        current_size: egui::Vec2,
        rdp_state: &mut RdpConnectionState,
    ) {
        if rdp_state.last_resize.borrow().elapsed().as_millis() < 500 {
            return;
        }
        let egui::Vec2 {
            x: actual_width,
            y: actual_height,
        } = current_size;
        // Get actual size, but must be 4-aligned
        let (actual_width, actual_height) = (actual_width as i32, actual_height as i32);
        let (actual_width, actual_height) = (
            actual_width - (actual_width % 4),
            actual_height - (actual_height % 4),
        );
        // Gdi will always be 4-aligned
        let (gdi_width, gdi_height) = unsafe { ((*rdp_state.gdi).width, (*rdp_state.gdi).height) };

        if actual_width != gdi_width || actual_height != gdi_height {
            // We only allow fullscreen/windowed, so if actual is smalleer we have exit the fullscreen mode
            // else, we have entered.
            if actual_width <= gdi_width {
                rdp_state.full_screen.store(false, Ordering::Relaxed);
            } else {
                rdp_state.full_screen.store(true, Ordering::Relaxed);
                // Store fullscreen for future possible use
                if self.screen_size.is_none() {
                    self.screen_size = Some((actual_width as u32, actual_height as u32))
                }
            }
            log::debug!(
                "Viewport size changed: actual=({}, {}), gdi=({}, {}), resizing gdi and texture",
                actual_width,
                actual_height,
                gdi_width,
                gdi_height
            );
            *rdp_state.last_resize.borrow_mut() = std::time::Instant::now();
            if let Some(disp) = rdp_state.channels.write().unwrap().disp() {
                disp.send_monitor_layout(
                    rdp::geom::Rect::new(0, 0, actual_width as u32, actual_height as u32),
                    0,
                    100, // in percent
                    100, // in percent
                );
                rdp_state.screen.resize_screen_texture(egui::Vec2::new(
                    actual_width as f32,
                    actual_height as f32,
                ));
            }
        }
    }
}
