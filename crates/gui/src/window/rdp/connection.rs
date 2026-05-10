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
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{log, logo::load_logo};
use anyhow::Result;
use eframe::egui;
use flume::{Receiver, Sender, bounded};

use rdp::{Rdp, messaging::RdpMessage, settings::RdpSettings, sys::rdpGdi};

use crate::window::{AppWindow, types::AppState};

const FRAMES_IN_FLIGHT: usize = 128;

#[derive(Clone)]
pub struct RemoteWindow {
    pub id: u32,
    pub owner_id: Option<u32>,
    pub style: Option<u32>,
    pub extended_style: Option<u32>,
    pub taskbar_button: Option<bool>,
    pub title: String,
    pub show_state: Option<u8>,
    pub is_offscreen: bool,
    pub rect: rdp::geom::Rect,
    pub resize_requested: bool,
    pub move_requested: bool,
    pub last_focused: bool,
    pub texture: Option<egui::TextureHandle>,
}

// Arcs are to keep original references when cloning
// because states are cloned when switching app states
#[derive(Clone)]
pub struct RdpConnectionState {
    pub update_rx: Receiver<RdpMessage>,
    pub gdi: *mut rdpGdi,
    pub gdi_lock: Arc<RwLock<()>>,
    pub channels: Arc<RwLock<rdp::channels::RdpChannels>>,
    pub screen: super::graphics::Screen,
    pub cursor: Rc<RefCell<super::mouse::RdpMouseCursor>>,
    pub full_screen: Rc<AtomicBool>,
    // For top pinbar
    pub pinbar_visible: Rc<AtomicBool>,

    // For resize, to avoiid too fast resizes
    pub last_resize: Rc<RefCell<std::time::Instant>>,

    pub command_tx: rdp::commands::Sender,
    pub command_event: rdp::utils::SafeHandle,
    pub fps: Rc<RefCell<super::fps::Fps>>,

    // RAIL / RemoteApp mode
    pub is_rail: bool,
    pub remote_windows: Arc<RwLock<std::collections::HashMap<u32, RemoteWindow>>>,
    pub scale_factor: f64,
    pub mouse_capture: Arc<Mutex<Option<u32>>>, // window_id being captured
    pub desktop_size: (u32, u32),
}

impl RdpConnectionState {
    pub fn toggle_fullscreen(&self, ctx: &egui::Context) {
        if self.is_rail {
            return;
        }
        let current = self.full_screen.load(Ordering::Relaxed);
        log::debug!("Toggling fullscreen from {}", current);
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!current));
        self.full_screen.store(!current, Ordering::Relaxed);
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub(crate) struct SafeInputPtr(pub(crate) *mut rdp::sys::rdpInput);
unsafe impl Send for SafeInputPtr {}
unsafe impl Sync for SafeInputPtr {}

impl fmt::Debug for RdpConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RdpState").field("gdi", &self.gdi).finish()
    }
}

impl AppWindow {
    pub fn enter_rdp_connection(
        &mut self,
        ctx: &eframe::egui::Context,
        frame: &mut eframe::Frame,
        rdp_settings: RdpSettings,
    ) -> Result<()> {
        let (tx, rx): (Sender<RdpMessage>, Receiver<RdpMessage>) = bounded(FRAMES_IN_FLIGHT);

        let mut rdp_settings = rdp_settings;

        let scale_factor = ctx.native_pixels_per_point().unwrap_or(1.0) as f64;
        rdp_settings.scale_factor = scale_factor;

        let is_full_screen = if rdp_settings.screen_size.is_fullscreen() {
            let real_size = ctx.content_rect().size();
            rdp_settings.screen_size = rdp::geom::ScreenSize::Fixed(
                (real_size.x as f64 * scale_factor) as u32,
                (real_size.y as f64 * scale_factor) as u32,
            );
            true
        } else {
            false
        };
        log::info!(
            "RDP Negotiated Scale Factor: {}%",
            rdp_settings.scale_factor * 100.0
        );

        let use_rgba = !super::graphics::Screen::supports_bgra(frame);

        let is_rail = rdp_settings.rail_app.is_some();

        let (width, height) = crate::monitor::size(0).unwrap_or_else(|| {
            let screen_size = ctx.input(|i| i.content_rect().size());
            (
                (screen_size.x * scale_factor as f32) as u32,
                (screen_size.y * scale_factor as f32) as u32,
            )
        });

        if is_rail {
            // Instead of hiding the main window (which pauses the event loop and stops updates),
            // we make it small and show a status message.
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize([300.0, 100.0].into()));
            ctx.send_viewport_cmd(egui::ViewportCommand::Title("UDS RemoteApp".to_owned()));

            // For RAIL, we need the "virtual desktop" to be at least the size of our physical screen
            // to allow mouse events to reach windows positioned anywhere.
            // We use the primary monitor size (index 0) as the base.
            rdp_settings.screen_size = rdp::geom::ScreenSize::Fixed(width, height);
            log::info!(
                "RAIL: Virtual desktop size set to {}x{} (from monitor 0)",
                width,
                height
            );
        }

        let (mut rdp_instance, command_tx) = Rdp::new(rdp_settings, tx, use_rgba);
        let command_event = rdp_instance.get_command_event();

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
        // And the lock
        let gdi_lock = rdp.gdi_lock();

        let texture_size = egui::Vec2::new(unsafe { (*gdi).width as f32 }, unsafe {
            (*gdi).height as f32
        });

        let cursor_img = load_logo();
        let cursor_img_size = cursor_img.size;
        let cursor = ctx.load_texture("rdp_cursor", cursor_img, egui::TextureOptions::LINEAR);

        let scale_factor = ctx.native_pixels_per_point().unwrap_or(1.0) as f64;

        if is_full_screen && !is_rail {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }

        self.set_app_state(AppState::RdpConnected(RdpConnectionState {
            update_rx: rx,
            gdi,
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
            command_tx,
            command_event,
            fps: Rc::new(RefCell::new(super::fps::Fps::new())),
            is_rail,
            remote_windows: Arc::new(RwLock::new(std::collections::HashMap::new())),
            scale_factor,
            mouse_capture: Arc::new(Mutex::new(None)),
            desktop_size: (width, height),
        }));

        // Clear any stale keys
        while self.keys_rx.try_recv().is_ok() {}
        self.processing_events.store(true, Ordering::Relaxed);

        let processing_events = self.processing_events.clone();
        std::thread::spawn(move || {
            // Note: This may already be marked as launched from external RDP launcher
            // But ensure it is marked here as well (to allow using from other gui launchers as test app)
            connection::tasks::mark_internal_rdp_as_running();
            let res = rdp.run();
            connection::tasks::mark_internal_rdp_as_not_running();
            processing_events.store(false, Ordering::Relaxed);
            log::debug!("RDP thread exiting...");
            if let Err(e) = res {
                log::debug!("RDP thread ended with error: {}", e);
            } else {
                log::debug!("RDP thread ended.");
            }
        });

        Ok(())
    }

    pub fn update_rdp_connection(
        &mut self,
        ui: &mut egui::Ui,
        frame: &mut eframe::Frame,
        rdp_state: RdpConnectionState,
    ) {
        if rdp_state.is_rail {
            self.update_rdp_rail(ui, frame, rdp_state);
        } else {
            self.update_rdp_session(ui, frame, rdp_state);
        }
    }

    pub(crate) fn handle_screen_resize(
        &mut self,
        ctx: &egui::Context,
        current_size: egui::Vec2,
        rdp_state: &mut RdpConnectionState,
    ) {
        // Sync fullscreen flag from viewport state
        let is_fs = ctx.input(|i| i.viewport().fullscreen).unwrap_or(false);
        rdp_state.full_screen.store(is_fs, Ordering::Relaxed);
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
            // In RAIL mode, we don't want to shrink the virtual desktop to the main window size
            if rdp_state.is_rail {
                return;
            }

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
            rdp_state.desktop_size = (actual_width as u32, actual_height as u32);
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
