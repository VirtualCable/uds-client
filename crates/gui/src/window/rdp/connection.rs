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
use crossbeam::channel::{Receiver, Sender, bounded};
use eframe::{egui, glow};

use crate::{log, logo::load_logo};

use rdp::{
    Rdp,
    messaging::RdpMessage,
    settings::RdpSettings,
    sys::{rdpGdi, rdpInput},
};

use crate::window::{
    AppWindow,
    types::{AppState, HotKey},
};

const FRAMES_IN_FLIGHT: usize = 128;

// Arcs are to keep original references when cloning
// because states are cloned when switching app states
#[derive(Clone)]
pub struct RdpConnectionState {
    pub update_rx: crossbeam::channel::Receiver<RdpMessage>,
    pub gdi: *mut rdpGdi,
    pub gdi_lock: Arc<RwLock<()>>,
    pub input: *mut rdpInput,
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

        // Rdp shouls be pinned, as build() inserts self reference inside freedrp structs
        let mut rdp: std::pin::Pin<Box<Rdp>> = Box::pin(Rdp::new(rdp_settings, tx, use_rgba));

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
            input,
            channels: rdp.channels().clone(),
            gdi_lock,
            screen: super::graphics::Screen::new(frame, texture_size, use_rgba),
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

    pub fn update_rdp_connection(
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

        self.handle_screen_resize(gl, ctx.content_rect().size(), &mut rdp_state);

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
                            self.set_custom_cursor(
                                ctx,
                                &mut rdp_state,
                                &data,
                                rdp::geom::Rect {
                                    x,
                                    y,
                                    w: width,
                                    h: height,
                                },
                            );
                        }
                    }
                }
                let screen = rdp_state.screen.clone();
                screen.update_screen_texture(gl, rects_to_update, rdp_state.clone());
                log::trace!("RDP update processing took {:?}", start.elapsed());
                // Show the texture on 0,0, full size
                let size = ui.available_size();
                ui.add_sized(
                    size,
                    egui::Image::new(egui::load::SizedTexture::new(screen.texture_id(), size)),
                );

                log::trace!("RDP frame rendered took {:?}", start.elapsed());
            });
        // Pinbar at top
        self.show_pinbar(ctx, &mut rdp_state);

        rdp_state.fps.borrow_mut().record_frame();
        // Handle custom cursor
        self.handle_cursor(ctx, &rdp_state);

        // Fps if enabled, last so it goes on top
        rdp_state.fps.borrow().show(ctx);
    }

    fn handle_hotkeys(&mut self, ctx: &egui::Context, rdp_state: &mut RdpConnectionState) -> bool {
        match HotKey::from_input(ctx) {
            HotKey::ToggleFullScreen => {
                self.toggle_fullscreen(ctx, rdp_state);
                true
            }
            HotKey::Skip => true,
            HotKey::ToggleFPS => {
                rdp_state.fps.borrow_mut().toggle();
                true
            }
            HotKey::None => false,
        }
    }

    fn handle_screen_resize(
        &mut self,
        gl: &glow::Context,
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
                let mut screen = rdp_state.screen.clone();
                screen.resize_screen_texture(gl, current_size);
            }
        }
    }
}
