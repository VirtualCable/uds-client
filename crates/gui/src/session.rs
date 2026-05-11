// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::collections::HashMap;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use flume::{Receiver, bounded};
use rdp::messaging::RdpMessage;
use rdp::settings::RdpSettings;
use shared::log;

use crate::RawKey;

const FRAMES_IN_FLIGHT: usize = 128;

// ── RDP Window ──────────────────────────────────────────────

#[allow(dead_code)]
pub struct RdpWindow {
    pub window: Arc<winit::window::Window>,
    pub renderer: crate::wgpu_render::WgpuRenderer,
    pub scratch: Vec<u8>,
}

// ── RAIL State ──────────────────────────────────────────────

#[allow(dead_code)]
pub struct RailWindow {
    pub id: u32,
    pub window: Arc<winit::window::Window>,
    pub rgba_data: Option<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

#[allow(dead_code)]
pub struct RailState {
    pub windows: HashMap<winit::window::WindowId, RailWindow>,
    pub visible_windows: HashMap<winit::window::WindowId, u32>,
    pub mouse_capture: Option<u32>,
    pub last_focused: HashMap<winit::window::WindowId, u32>,
}

// ── RDP State ───────────────────────────────────────────────

#[allow(dead_code)]
pub struct RdpState {
    pub window: RdpWindow,
    pub update_rx: Receiver<RdpMessage>,
    pub gdi: *mut rdp::sys::rdpGdi,
    pub gdi_lock: Arc<RwLock<()>>,
    pub channels: Arc<RwLock<rdp::channels::RdpChannels>>,
    pub command_tx: rdp::commands::Sender,
    pub command_event: rdp::utils::SafeHandle,
    pub is_rail: bool,
    pub scale_factor: f64,
    pub desktop_size: (u32, u32),
    pub full_screen: Arc<AtomicBool>,
    pub last_resize: std::time::Instant,
    pub pending_resize: bool,
    pub pinbar_visible: bool,
    pub keys_rx: Receiver<RawKey>,
    pub fps: Fps,

    pub rail: Option<RailState>,
    pub rail_channel: Option<rdp::channels::rail::RailChannel>,

    pub cursor_data: Vec<u8>,
    pub cursor_hot_x: u32,
    pub cursor_hot_y: u32,
    pub cursor_w: u32,
    pub cursor_h: u32,
    pub cursor_visible: bool,
    pub cursor_x: f32,
    pub cursor_y: f32,
}

// ── FPS Counter ─────────────────────────────────────────────

#[allow(dead_code)]
pub struct Fps {
    pub last_instant: std::time::Instant,
    frames: Vec<f32>,
    pub enabled: AtomicBool,
}

impl Fps {
    pub fn new() -> Self {
        Self {
            last_instant: std::time::Instant::now(),
            frames: Vec::new(),
            enabled: AtomicBool::new(false),
        }
    }
    pub fn record(&mut self) {
        let delta = self.last_instant.elapsed().as_secs_f32();
        self.last_instant = std::time::Instant::now();
        self.frames.push(delta);
        if self.frames.len() > 128 {
            self.frames.remove(0);
        }
    }
    pub fn toggle(&self) {
        let v = self.enabled.load(Ordering::Relaxed);
        self.enabled.store(!v, Ordering::Relaxed);
    }
    pub fn average(&self) -> f32 {
        let count = self.frames.len();
        if count < 2 {
            return 0.0;
        }
        let sum: f32 = self.frames.iter().sum();
        sum / count as f32
    }
}

// ── RDP Action Result ───────────────────────────────────────

#[allow(dead_code)]
pub enum RdpActionResult {
    Continue,
    Skip,
    Disconnect,
    Error(String),
}

// ── RdpState impl ───────────────────────────────────────────

impl RdpState {
    pub fn new(
        window: RdpWindow,
        settings: RdpSettings,
        is_rail: bool,
        scale_factor: f64,
        desktop_size: (u32, u32),
        keys_rx: Receiver<RawKey>,
        use_rgba: bool,
    ) -> Result<Self> {
        let (tx, rx) = bounded::<RdpMessage>(FRAMES_IN_FLIGHT);

        let mut rdp_settings = settings;
        rdp_settings.scale_factor = scale_factor;

        if is_rail {
            rdp_settings.screen_size = rdp::geom::ScreenSize::Fixed(desktop_size.0, desktop_size.1);
        }

        let (mut rdp_instance, command_tx) = rdp::Rdp::new(rdp_settings, tx, use_rgba);
        let command_event = rdp_instance.get_command_event();

        if is_rail {
            rdp_instance.set_window_callbacks(vec![
                rdp::callbacks::window_c::Callbacks::Create,
                rdp::callbacks::window_c::Callbacks::Update,
                rdp::callbacks::window_c::Callbacks::Delete,
            ]);
        }

        let mut rdp = Box::pin(rdp_instance);
        rdp.as_mut().build()?;
        rdp.connect()?;

        let gdi = rdp
            .gdi()
            .ok_or_else(|| anyhow::anyhow!("GDI not initialized"))?;
        let gdi_lock = rdp.gdi_lock();
        let channels = rdp.channels().clone();

        log::info!(
            "RDP connected: GDI={}x{} stride={}",
            unsafe { (*gdi).width },
            unsafe { (*gdi).height },
            unsafe { (*gdi).stride }
        );

        let rail_channel = if is_rail {
            channels.read().unwrap().rail()
        } else {
            None
        };

        std::thread::spawn(move || {
            connection::tasks::mark_internal_rdp_as_running();
            let res = rdp.run();
            connection::tasks::mark_internal_rdp_as_not_running();
            if let Err(e) = res {
                log::error!("RDP thread error: {}", e);
            }
        });

        Ok(RdpState {
            window,
            update_rx: rx,
            gdi,
            gdi_lock,
            channels,
            command_tx,
            command_event,
            is_rail,
            scale_factor,
            desktop_size,
            full_screen: Arc::new(AtomicBool::new(false)),
            last_resize: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(60))
                .unwrap_or(std::time::Instant::now()),
            pending_resize: false,
            pinbar_visible: false,
            keys_rx,
            fps: Fps::new(),
            rail: None,
            rail_channel,
            cursor_data: Vec::new(),
            cursor_hot_x: 0,
            cursor_hot_y: 0,
            cursor_w: 0,
            cursor_h: 0,
            cursor_visible: false,
            cursor_x: 0.0,
            cursor_y: 0.0,
        })
    }

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

            // Copy GDI → scratch with optional swizzle
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
        } // lock dropped

        // Build overlays: cursor, FPS, pinbar
        let mut overlays: Vec<crate::wgpu_render::OverlayParams> = Vec::new();

        if self.cursor_visible && !self.cursor_data.is_empty() {
            let sf = self.scale_factor as f32;
            overlays.push(crate::wgpu_render::OverlayParams {
                rgba: self.cursor_data.as_slice(),
                width: self.cursor_w,
                height: self.cursor_h,
                x: self.cursor_x - self.cursor_hot_x as f32 * sf,
                y: self.cursor_y - self.cursor_hot_y as f32 * sf,
                scale: sf,
            });
        }

        // Build text sections for FPS + pinbar
        let mut text_sections: Vec<crate::wgpu_render::OwnedSection> = Vec::new();
        let phys = self.window.window.inner_size();

        if self.fps.enabled.load(Ordering::Relaxed) {
            let fps_text = format!("FPS: {:.1}", self.fps.average());
            let section = crate::wgpu_render::Section::default()
                .add_text(
                    crate::wgpu_render::Text::new(&fps_text)
                        .with_scale(14.0)
                        .with_color([1.0, 1.0, 1.0, 1.0]),
                )
                .with_screen_position(((phys.width - 100) as f32, 8.0))
                .to_owned();
            text_sections.push(section);
        }

        if self.pinbar_visible {
            let section = crate::wgpu_render::Section::default()
                .add_text(
                    crate::wgpu_render::Text::new("UDS Connection      [ \u{2B1C} ]  [ X ]")
                        .with_scale(14.0)
                        .with_color([1.0, 1.0, 1.0, 1.0]),
                )
                .with_screen_position(((phys.width / 2) as f32 - 100.0, 4.0))
                .to_owned();
            text_sections.push(section);
        }

        self.window.renderer.update_and_render(
            &self.window.scratch,
            fb_w as u32,
            fb_h as u32,
            &overlays,
            &text_sections,
        );

        Ok(())
    }

    /// Called when window size changes (fullscreen toggle, manual resize).
    /// Sends the new logical resolution to the RDP server.
    /// The server responds with DesktopResize when done.
    pub fn request_screen_resize(&mut self) {
        if self.last_resize.elapsed().as_millis() < 500 {
            return;
        }
        let phys = self.window.window.inner_size();
        let sf = self.scale_factor.max(1.0);
        let rdp_w = ((phys.width as f64 / sf) as u32).max(1) & !3;
        let rdp_h = ((phys.height as f64 / sf) as u32).max(1) & !3;

        log::info!(
            "request_screen_resize: phys={}x{} → rdp={rdp_w}x{rdp_h} (scale={sf})",
            phys.width,
            phys.height
        );

        self.window.renderer.reconfigure(phys.width, phys.height);
        self.last_resize = std::time::Instant::now();
        self.pending_resize = true;

        if let Some(disp) = self.channels.write().unwrap().disp() {
            disp.send_monitor_layout(rdp::geom::Rect::new(0, 0, rdp_w, rdp_h), 0, 100, 100);
        }
    }

    /// Called when the server acknowledges the resize via DesktopResize message
    pub fn on_desktop_resize(&mut self, _width: u32, _height: u32) {
        log::info!("DesktopResize acknowledged: {_width}x{_height}");
        self.pending_resize = false;
        // The GDI has already been updated by FreeRDP internally
        // Just reconfigure the wgpu surface in case it changed
        let phys = self.window.window.inner_size();
        self.window.renderer.reconfigure(phys.width, phys.height);
    }
}

/// Process an RDP message, returning the action to take
pub fn handle_rdp_message(state: &mut RdpState, message: RdpMessage) -> RdpActionResult {
    log::trace!("RDP message: {:?}", message);
    match message {
        RdpMessage::UpdateRects(_rects) => RdpActionResult::Continue,
        RdpMessage::DesktopResize(w, h) => {
            state.on_desktop_resize(w, h);
            RdpActionResult::Continue
        }
        RdpMessage::Disconnect => RdpActionResult::Disconnect,
        RdpMessage::Error(e) => {
            log::error!("RDP Error: {}", e);
            RdpActionResult::Error(e)
        }
        RdpMessage::WindowCreate {
            window_id,
            title,
            pos: _,
            size: _,
            ..
        } if state.is_rail => {
            log::info!("RAIL window created: {} ({})", window_id, title);
            RdpActionResult::Skip
        }
        RdpMessage::WindowDelete(window_id) if state.is_rail => {
            log::info!("RAIL window deleted: {}", window_id);
            if let Some(rail) = &mut state.rail {
                rail.visible_windows.retain(|_, &mut v| v != window_id);
            }
            RdpActionResult::Skip
        }
        RdpMessage::WindowPixels {
            window_id,
            width,
            height,
            data,
        } if state.is_rail => {
            if let Some(rail) = &mut state.rail {
                for rw in rail.windows.values_mut() {
                    if rw.id == window_id {
                        rw.rgba_data = Some(data);
                        rw.width = width;
                        rw.height = height;
                        rw.window.request_redraw();
                        break;
                    }
                }
            }
            RdpActionResult::Skip
        }
        RdpMessage::SetCursorIcon(data, x, y, width, height) => {
            state.cursor_data = data;
            state.cursor_hot_x = x;
            state.cursor_hot_y = y;
            state.cursor_w = width;
            state.cursor_h = height;
            state.cursor_visible = width > 0 && height > 0;
            RdpActionResult::Continue
        }
        _ => RdpActionResult::Skip,
    }
}
