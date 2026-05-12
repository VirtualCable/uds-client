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
use crate::monitor;

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
    pub renderer: Option<crate::wgpu_render::WgpuRenderer>,
    pub rgba_data: Option<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub rect: rdp::geom::Rect,
    pub title: String,
    pub show_in_taskbar: bool,
    pub has_decorations: bool,
    pub last_focused: bool,
    pub offscreen: bool,
}

#[allow(dead_code)]
pub struct RailState {
    pub windows: HashMap<winit::window::WindowId, u32>, // WindowId → rail window_id
    pub mouse_capture: Option<u32>,
}

/// Pending RAIL action to be executed by the event loop
pub enum RailAction {
    Create(u32, String, rdp::geom::Rect, bool, bool), // id, title, rect, taskbar, decorations
    Delete(u32),                                      // window_id
    UpdatePosition(u32, rdp::geom::Rect),
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
    pub last_windowed_size: Option<(u32, u32)>,
    pub last_resize: std::time::Instant,
    pub pending_resize: bool,
    pub pinbar_visible: bool,
    pub pinbar_rect: Option<(u32, u32)>,
    pub pinbar_btn_fs_x: std::ops::Range<f32>,
    pub pinbar_btn_close_x: std::ops::Range<f32>,
    pub keys_rx: Receiver<RawKey>,
    pub fps: Fps,

    pub rail: RailState,
    pub rail_channel: Option<rdp::channels::rail::RailChannel>,
    pub rail_actions: Vec<RailAction>,
    pub rail_windows: HashMap<u32, RailWindow>, // id → RailWindow

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
    frames: Vec<std::time::Instant>,
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
        let now = std::time::Instant::now();
        // Discard frames older than 2 seconds
        self.frames
            .retain(|t| now.duration_since(*t).as_secs_f32() < 2.0);
        self.frames.push(now);
    }
    pub fn toggle(&self) {
        let v = self.enabled.load(Ordering::Relaxed);
        self.enabled.store(!v, Ordering::Relaxed);
    }
    pub fn average(&self) -> f32 {
        let now = std::time::Instant::now();
        let recent: Vec<_> = self
            .frames
            .iter()
            .filter(|t| now.duration_since(**t).as_secs_f32() < 1.0)
            .collect();
        recent.len() as f32
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
            last_windowed_size: None,
            last_resize: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(60))
                .unwrap_or(std::time::Instant::now()),
            pending_resize: false,
            pinbar_visible: false,
            pinbar_rect: None,
            pinbar_btn_fs_x: 0.0..0.0,
            pinbar_btn_close_x: 0.0..0.0,
            keys_rx,
            fps: Fps::new(),
            rail: RailState {
                windows: HashMap::new(),
                mouse_capture: None,
            },
            rail_channel,
            rail_actions: Vec::new(),
            rail_windows: HashMap::new(),
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

        // Build overlays: glass backgrounds
        let mut overlays: Vec<crate::wgpu_render::OverlayParams> = Vec::new();

        // Cursor overlay (drawn on top of everything)
        let cursor_overlay = if self.cursor_visible && !self.cursor_data.is_empty() {
            let sf = self.scale_factor;
            let (hot_x, hot_y) =
                monitor::logic_2_phys_pos((self.cursor_hot_x as i32, self.cursor_hot_y as i32), sf);
            Some(crate::wgpu_render::OverlayParams {
                rgba: self.cursor_data.as_slice(),
                width: self.cursor_w,
                height: self.cursor_h,
                x: self.cursor_x - hot_x as f32,
                y: self.cursor_y - hot_y as f32,
                scale: sf as f32,
            })
        } else {
            None
        };

        // Build text sections + backgrounds for FPS + pinbar
        let mut text_sections: Vec<crate::wgpu_render::OwnedSection> = Vec::new();
        let phys = self.window.window.inner_size();
        let mut _ov_data: Vec<Vec<u8>> = Vec::new();
        struct OvDesc {
            data_idx: usize,
            w: u32,
            h: u32,
            x: f32,
            y: f32,
            scale: f32,
        }
        let mut ov_descs: Vec<OvDesc> = Vec::new();

        if self.fps.enabled.load(Ordering::Relaxed) {
            let fps_bg = include_bytes!("images/fps.png");
            let (bg_rgba, bw, bh) = crate::draw::load_png_rgba(fps_bg);
            // PNG at 2x → physical at monitor scale: (w/2)*scale, (h/2)*scale
            let bg_w = monitor::scaled_val((bw as i32 / 2).max(1)) as u32;
            let _bg_h = monitor::scaled_val((bh as i32 / 2).max(1)) as u32;
            let margin = monitor::scaled_val(8) as u32;
            let x = phys.width.saturating_sub(bg_w + margin) as f32;
            let y = margin as f32;
            let idx = _ov_data.len();
            _ov_data.push(bg_rgba);
            let scale = bg_w as f32 / bw as f32;
            ov_descs.push(OvDesc {
                data_idx: idx,
                w: bw,
                h: bh,
                x,
                y,
                scale,
            });
            // FPS value at (26, 4) in 1x coords (PNG 52/2, 8/2)
            let fps_text = format!("{:.0}", self.fps.average());
            let font_size = monitor::scaled_val(12) as f32;
            text_sections.push(
                crate::wgpu_render::Section::default()
                    .add_text(
                        crate::wgpu_render::Text::new(&fps_text)
                            .with_scale(font_size)
                            .with_color([1.0, 1.0, 1.0, 1.0]),
                    )
                    .with_screen_position((
                        x + monitor::scaled_val(26) as f32,
                        y + monitor::scaled_val(4) as f32,
                    ))
                    .to_owned(),
            );
        }

        if self.pinbar_visible {
            let pinbar_bg = include_bytes!("images/pinbar.png");
            let (bg_rgba, bw, bh) = crate::draw::load_png_rgba(pinbar_bg);
            let bg_w = monitor::scaled_val(bw as i32) as u32;
            let bg_h = monitor::scaled_val(bh as i32) as u32;
            let x = (phys.width.saturating_sub(bg_w) / 2) as f32;
            let idx = _ov_data.len();
            _ov_data.push(bg_rgba);
            let scale = *monitor::SCALE_FACTOR as f32;
            ov_descs.push(OvDesc {
                data_idx: idx,
                w: bw,
                h: bh,
                x,
                y: 0.0,
                scale,
            });
            // Label at (8, 8)
            let font_size = monitor::scaled_val(16) as f32;
            text_sections.push(
                crate::wgpu_render::Section::default()
                    .add_text(
                        crate::wgpu_render::Text::new("UDS Connection")
                            .with_scale(font_size)
                            .with_color([1.0, 1.0, 1.0, 1.0]),
                    )
                    .with_screen_position((
                        x + monitor::scaled_val(8) as f32,
                        monitor::scaled_val(8) as f32,
                    ))
                    .to_owned(),
            );
            // Click areas from PNG coords: fs=(220,8)-(239,28), close=(243,8)-(262,28)
            self.pinbar_btn_fs_x =
                (x + monitor::scaled_val(220) as f32)..(x + monitor::scaled_val(239) as f32);
            self.pinbar_btn_close_x =
                (x + monitor::scaled_val(243) as f32)..(x + monitor::scaled_val(262) as f32);
            self.pinbar_rect = Some((bg_w, bg_h));
        }

        // Phase 2: build overlays from stable data
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

    /// Called when window size changes (fullscreen toggle, manual resize).
    /// Sends the new logical resolution to the RDP server.
    /// The server responds with DesktopResize when done.
    pub fn request_screen_resize(&mut self) {
        if self.last_resize.elapsed().as_millis() < 500 {
            return;
        }
        let phys = self.window.window.inner_size();
        let sf = self.scale_factor.max(1.0);
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
            owner_id,
            title,
            pos,
            size,
            taskbar_button,
            ext_style,
            is_offscreen,
            show_state,
            ..
        } if state.is_rail => {
            // Skip transparent overlay/shadow windows (WS_EX_TRANSPARENT = 0x20)
            if ext_style.is_some_and(|s| (s & 0x20) != 0) {
                return RdpActionResult::Continue;
            }
            let sf = state.scale_factor.max(1.0);
            let (x, y) = pos.unwrap_or((0, 0));
            let (w, h) = size.unwrap_or((0, 0));
            let rect = rdp::geom::Rect::new(
                (x as f64 / sf) as i32,
                (y as f64 / sf) as i32,
                (w as f64 / sf) as u32,
                (h as f64 / sf) as u32,
            );
            let is_tool = ext_style.is_some_and(|s| (s & 0x80) != 0);
            let has_owner = owner_id.is_some() && owner_id != Some(0);
            let show_taskbar = taskbar_button.unwrap_or(!is_tool && !has_owner);
            let hidden = show_state == Some(0);
            if !hidden && rect.w > 0 && rect.h > 0 && !is_offscreen.unwrap_or(false) {
                state.rail_actions.push(RailAction::Create(
                    window_id,
                    title,
                    rect,
                    show_taskbar,
                    false,
                ));
            }
            RdpActionResult::Continue
        }
        RdpMessage::WindowUpdate {
            window_id,
            pos,
            size,
            is_offscreen,
            show_state,
            ..
        } if state.is_rail => {
            if is_offscreen.unwrap_or(false) || show_state == Some(0) {
                state.rail_actions.push(RailAction::Delete(window_id));
            } else if let (Some((x, y)), Some((w, h))) = (pos, size) {
                let sf = state.scale_factor.max(1.0);
                let rect = rdp::geom::Rect::new(
                    (x as f64 / sf) as i32,
                    (y as f64 / sf) as i32,
                    (w as f64 / sf) as u32,
                    (h as f64 / sf) as u32,
                );
                state
                    .rail_actions
                    .push(RailAction::UpdatePosition(window_id, rect));
            }
            RdpActionResult::Continue
        }
        RdpMessage::WindowDelete(window_id) if state.is_rail => {
            state.rail_actions.push(RailAction::Delete(window_id));
            RdpActionResult::Continue
        }
        RdpMessage::WindowPixels {
            window_id,
            width,
            height,
            data,
        } if state.is_rail => {
            if let Some(rw) = state.rail_windows.get_mut(&window_id) {
                let sf = state.scale_factor.max(1.0);
                let lw = ((width as f64 / sf) as u32).min(state.desktop_size.0);
                let lh = ((height as f64 / sf) as u32).min(state.desktop_size.1);
                if rw.rect.w != lw || rw.rect.h != lh {
                    rw.rect.w = lw;
                    rw.rect.h = lh;
                    let _ = rw.window.request_inner_size(winit::dpi::LogicalSize::new(
                        lw as f64,
                        lh as f64,
                    ));
                    if let Some(ref mut renderer) = rw.renderer {
                        let phys = rw.window.inner_size();
                        renderer.reconfigure(phys.width, phys.height);
                    }
                }
                rw.rgba_data = Some(data);
                rw.width = width;
                rw.height = height;
                rw.window.request_redraw();
            }
            RdpActionResult::Continue
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
