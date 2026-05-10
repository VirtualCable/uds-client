// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::{Arc, RwLock, atomic::AtomicBool};

use anyhow::Result;
use flume::{Receiver, bounded};
use rdp::messaging::RdpMessage;
use rdp::settings::RdpSettings;
use shared::log;

use crate::RawKey;

const FRAMES_IN_FLIGHT: usize = 128;

// ── RDP Window ──────────────────────────────────────────────

pub struct RdpWindow {
    pub window: Arc<winit::window::Window>,
    pub surface: softbuffer::Surface<Arc<winit::window::Window>, Arc<winit::window::Window>>,
    pub context: softbuffer::Context<Arc<winit::window::Window>>,
    pub scratch: Vec<u8>,
}

// ── RAIL State ──────────────────────────────────────────────

pub struct RailWindow {
    pub id: u32,
    pub window: Arc<winit::window::Window>,
    pub surface: softbuffer::Surface<Arc<winit::window::Window>, Arc<winit::window::Window>>,
    pub _context: softbuffer::Context<Arc<winit::window::Window>>,
    pub rgba_data: Option<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

pub struct RailState {
    pub windows: HashMap<winit::window::WindowId, RailWindow>,
    pub visible_windows: HashMap<winit::window::WindowId, u32>,
    pub mouse_capture: Option<u32>,
    pub last_focused: HashMap<winit::window::WindowId, u32>,
}

// ── RDP State ───────────────────────────────────────────────

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
    pub keys_rx: Receiver<RawKey>,
    pub fps: Fps,

    pub rail: Option<RailState>,
    pub rail_channel: Option<rdp::channels::rail::RailChannel>,
}

// ── FPS Counter ─────────────────────────────────────────────

pub struct Fps {
    pub last_instant: std::time::Instant,
    frames: Vec<f32>,
    pub enabled: bool,
}

impl Fps {
    pub fn new() -> Self {
        Self {
            last_instant: std::time::Instant::now(),
            frames: Vec::new(),
            enabled: false,
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
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }
}

// ── RDP Action Result ───────────────────────────────────────

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
            keys_rx,
            fps: Fps::new(),
            rail: None,
            rail_channel,
        })
    }

    pub fn update_screen(&mut self) -> Result<()> {
        let gdi = self.gdi;
        let _lock = self.gdi_lock.read().unwrap();

        let (stride, fb_h, fb_w) = unsafe {
            (
                (*gdi).stride as usize,
                (*gdi).height as usize,
                (*gdi).width as usize,
            )
        };

        if fb_w == 0 || fb_h == 0 {
            return Ok(());
        }

        let cw = fb_w as u32;
        let ch = fb_h as u32;
        if cw > 0 && ch > 0 {
            let _ = self
                .window
                .surface
                .resize(NonZeroU32::new(cw).unwrap(), NonZeroU32::new(ch).unwrap());
        }

        let framebuffer = unsafe {
            std::slice::from_raw_parts((*gdi).primary_buffer as *const u8, stride * fb_h)
        };

        if let Ok(mut buffer) = self.window.surface.buffer_mut() {
            let need_swizzle = cfg!(target_os = "macos");
            let dst = buffer.as_mut();
            for row in 0..fb_h.min(dst.len() as usize / fb_w.max(1)) {
                let src_start = row * stride;
                let dst_start = row * fb_w.max(1);
                for col in 0..fb_w {
                    let si = src_start + col * 4;
                    if si + 3 < framebuffer.len() {
                        let b = framebuffer[si];
                        let g = framebuffer[si + 1];
                        let r = framebuffer[si + 2];
                        let a = framebuffer[si + 3];
                        let di = dst_start + col;
                        if di < dst.len() {
                            if need_swizzle {
                                dst[di] = u32::from_ne_bytes([r, g, b, a]);
                            } else {
                                dst[di] = u32::from_ne_bytes([b, g, r, a]);
                            }
                        }
                    }
                }
            }
            let _ = buffer.present();
        }

        Ok(())
    }

    pub fn update_screen_rects(&mut self, rects: &[rdp::geom::Rect]) {
        let gdi = self.gdi;
        let _lock = self.gdi_lock.read().unwrap();

        let (stride, fb_h, fb_w) = unsafe {
            (
                (*gdi).stride as usize,
                (*gdi).height as usize,
                (*gdi).width as usize,
            )
        };

        let framebuffer = unsafe {
            std::slice::from_raw_parts((*gdi).primary_buffer as *const u8, stride * fb_h)
        };

        let need_swizzle = cfg!(target_os = "macos");

        if let Ok(mut buffer) = self.window.surface.buffer_mut() {
            let dst = buffer.as_mut();
            for rect in rects {
                let rx = rect.x.max(0) as usize;
                let ry = rect.y.max(0) as usize;
                let rw = rect.w as usize;
                let rh = rect.h as usize;

                for row in 0..rh {
                    let py = ry + row;
                    if py >= fb_h {
                        break;
                    }
                    let src_start = py * stride + rx * 4;
                    let dst_start = py * fb_w.max(1) + rx;
                    for col in 0..rw {
                        let si = src_start + col * 4;
                        if si + 3 < framebuffer.len() && dst_start + col < dst.len() {
                            let b = framebuffer[si];
                            let g = framebuffer[si + 1];
                            let r = framebuffer[si + 2];
                            let a = framebuffer[si + 3];
                            if need_swizzle {
                                dst[dst_start + col] = u32::from_ne_bytes([r, g, b, a]);
                            } else {
                                dst[dst_start + col] = u32::from_ne_bytes([b, g, r, a]);
                            }
                        }
                    }
                }
            }
            let _ = buffer.present();
        }
    }
}

/// Process an RDP message, returning the action to take
pub fn handle_rdp_message(state: &mut RdpState, message: RdpMessage) -> RdpActionResult {
    match message {
        RdpMessage::UpdateRects(rects) => {
            if !state.is_rail {
                state.update_screen_rects(&rects);
            }
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
            pos,
            size,
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
                        let _ = rw.window.request_redraw();
                        break;
                    }
                }
            }
            RdpActionResult::Skip
        }
        RdpMessage::SetCursorIcon(_data, _x, _y, _width, _height) => RdpActionResult::Skip,
        _ => RdpActionResult::Skip,
    }
}
