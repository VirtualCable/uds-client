// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

mod cursor;
mod fps;
mod pinbar;
mod rail;
mod session;

pub use cursor::Cursor;
pub use fps::Fps;
pub use pinbar::Pinbar;
pub use rail::{RailAction, RailState, RailWindow};

use std::collections::HashMap;
use std::sync::{Arc, RwLock, atomic::AtomicBool};

use anyhow::Result;
use flume::{Receiver, bounded};
use rdp_ffi::messaging::RdpMessage;
use rdp_ffi::settings::RdpSettings;
use shared::log;

use crate::RawKey;

const FRAMES_IN_FLIGHT: usize = 128;

#[allow(dead_code)]
pub struct RdpWindow {
    pub window: Arc<winit::window::Window>,
    pub renderer: crate::wgpu_render::WgpuRenderer,
    pub scratch: Vec<u8>,
}

pub struct Pendings {
    pub resize: bool,
    pub pixels: HashMap<u32, (u32, u32, Vec<u8>)>,
    pub icons: HashMap<u32, (Vec<u8>, u32, u32)>,
    pub rects: Vec<rdp_ffi::geom::Rect>,
}

pub struct Rail {
    pub state: RailState,
    pub channel: Option<rdp_ffi::channels::rail::RailChannel>,
    pub actions: Vec<RailAction>,
    pub windows: HashMap<u32, RailWindow>,
    pub control: Option<crate::draw::ui::rail_control::RailControl>,
}

pub enum RdpMode {
    Desktop {
        pinbar: Pinbar,
        full_screen: Arc<AtomicBool>,
        last_windowed_size: Option<(u32, u32)>,
        last_resize: std::time::Instant,
        fps: Fps,
    },
    Rail(Rail),
}

#[allow(dead_code)]
pub struct RdpState {
    pub window: RdpWindow,
    pub update_rx: Receiver<RdpMessage>,
    pub gdi: *mut rdp_ffi::sys::rdpGdi,
    pub gdi_lock: Arc<RwLock<()>>,
    pub channels: Arc<RwLock<rdp_ffi::channels::RdpChannels>>,
    pub command_tx: rdp_ffi::messaging::CommandSender,
    pub command_event: rdp_ffi::utils::SafeHandle,
    pub coords_scale: f64,
    pub desktop_size: (u32, u32),
    pub keys_rx: Receiver<RawKey>,

    pub mode: RdpMode,
    pub pendings: Pendings,

    pub cursor: Cursor,
}

#[allow(dead_code)]
pub enum RdpActionResult {
    Continue,
    Skip,
    Disconnect,
    Error(String),
}

impl RdpState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        window: RdpWindow,
        settings: RdpSettings,
        is_rail: bool,
        coords_scale: f64,
        desktop_size: (u32, u32),
        keys_rx: Receiver<RawKey>,
        use_rgba: bool,
        default_title: String,
        exit_text: String,
    ) -> Result<Self> {
        log::debug!(
            "RdpState::new: is_rail={}, coords_scale={}, desktop_size={:?}, use_rgba={}",
            is_rail,
            coords_scale,
            desktop_size,
            use_rgba
        );
        let (tx, rx) = bounded::<RdpMessage>(FRAMES_IN_FLIGHT);

        let scale_factor = settings.options.desktop_scale;
        let rail_title = settings.rail.as_ref().and_then(|r| r.title.clone());

        let integrations = rdp_ffi::integrations::RdpIntegrations {
            audio_output: Some(Arc::new(channels::audio::output::AudioHandle::new())),
            audio_input: Some(Arc::new(channels::audio::input::MicHandle::new())),
            webcam: Some(Arc::new(channels::webcam::WebcamHandle::new())),
            clipboard: Some(Arc::new(channels::clipboard::ClipboardHandle::new())),
        };

        let (rdp_instance, command_tx) =
            rdp_ffi::Rdp::new(settings, tx, use_rgba, None, integrations);
        let command_event = rdp_instance.get_command_event();

        let mut rdp = Box::pin(rdp_instance);
        rdp.as_mut().build()?;
        rdp.connect()?;

        let gdi = rdp
            .gdi()
            .ok_or_else(|| anyhow::anyhow!("GDI not initialized"))?;
        let gdi_lock = rdp.gdi_lock();
        let channels = rdp.channels().clone();

        log::info!(
            "RDP connected: GDI={}x{}, scale={}, stride={}",
            unsafe { (*gdi).width },
            unsafe { (*gdi).height },
            scale_factor,
            unsafe { (*gdi).stride }
        );

        let rail_channel = if is_rail {
            channels.read().unwrap().rail()
        } else {
            None
        };

        let rail_control = if is_rail {
            let phys = window.window.inner_size();
            let scale = *crate::monitor::SCALE_FACTOR as f32;
            let title = rail_title.unwrap_or(default_title);
            Some(crate::draw::ui::rail_control::RailControl::new(
                title,
                phys.width as f32,
                phys.height as f32,
                scale,
                exit_text,
            ))
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

        let mode = if is_rail {
            RdpMode::Rail(Rail {
                state: RailState {
                    windows: HashMap::new(),
                    mouse_capture: None,
                },
                channel: rail_channel,
                actions: Vec::new(),
                windows: HashMap::new(),
                control: rail_control,
            })
        } else {
            RdpMode::Desktop {
                pinbar: Pinbar::new(),
                full_screen: Arc::new(AtomicBool::new(false)),
                last_windowed_size: None,
                last_resize: std::time::Instant::now()
                    .checked_sub(std::time::Duration::from_secs(60))
                    .unwrap_or(std::time::Instant::now()),
                fps: Fps::new(),
            }
        };

        Ok(RdpState {
            window,
            update_rx: rx,
            gdi,
            gdi_lock,
            channels,
            command_tx,
            command_event,
            coords_scale,
            desktop_size,
            keys_rx,
            mode,
            pendings: Pendings {
                resize: false,
                pixels: HashMap::new(),
                icons: HashMap::new(),
                rects: Vec::new(),
            },
            cursor: Cursor::new(coords_scale, use_rgba),
        })
    }
}

/// Process an RDP message, returning the action to take
pub fn handle_rdp_message(state: &mut RdpState, message: RdpMessage) -> RdpActionResult {
    log::trace!("RDP message: {:?}", message);
    match message {
        RdpMessage::UpdateRects(rects) => {
            if matches!(state.mode, RdpMode::Desktop { .. }) {
                state.pendings.rects.extend(rects);
            }
            RdpActionResult::Continue
        }
        RdpMessage::DesktopResize(w, h) => {
            state.on_desktop_resize(w, h);
            RdpActionResult::Continue
        }
        RdpMessage::Disconnect => RdpActionResult::Disconnect,
        RdpMessage::Error(e) => {
            log::error!("RDP Error: {}", e);
            RdpActionResult::Error(e)
        }
        RdpMessage::SetCursorIcon(data, x, y, width, height) => {
            state.cursor.set_icon(data, x, y, width, height);
            RdpActionResult::Continue
        }
        // Delegate RAIL-specific messages
        ref msg if matches!(state.mode, RdpMode::Rail(_)) => {
            rail::handle_rail_message(state, msg.clone())
        }
        _ => RdpActionResult::Skip,
    }
}

use crate::WindowKind;
use std::sync::atomic::Ordering;
use winit::event_loop::ActiveEventLoop;

impl crate::AppHandler {
    pub(crate) fn open_rdp(
        &mut self,
        el: &ActiveEventLoop,
        mut settings: rdp_ffi::settings::RdpSettings,
    ) -> Result<()> {
        macro_rules! tr {
            ($msg:expr) => {
                self.gettext($msg)
            };
        }
        self.close_progress(); // Ensure progress is closed if it was open before
        let is_rail = settings.rail.is_some();
        let use_rgba = cfg!(target_os = "macos");

        let monitor_scale = crate::monitor::scale(0);
        let (desktop_w, desktop_h) = crate::monitor::size(0).unwrap_or((1920, 1080));
        let use_local_scaler = settings.options.use_local_scaler;
        let local_scale = if use_local_scaler { monitor_scale } else { 1.0 };

        let (rdp_w, rdp_h) = match (settings.screen_size, is_rail) {
            (rdp_ffi::geom::ScreenSize::Full, _) | (_, true) => {
                let (lw, lh) =
                    crate::monitor::phys_2_logic((desktop_w as i32, desktop_h as i32), local_scale);
                (lw as u32, lh as u32)
            }
            (rdp_ffi::geom::ScreenSize::Fixed(w, h), _) => (w, h),
        };
        let coords_scale = if use_local_scaler {
            settings.options.desktop_scale = 1.0;
            monitor_scale
        } else {
            settings.options.desktop_scale = monitor_scale;
            1.0
        };
        let desktop_size = (rdp_w, rdp_h);
        let is_fullscreen = settings.screen_size.is_fullscreen() && !is_rail;
        let (window_logical_w, window_logical_h) = if let rdp_ffi::geom::ScreenSize::Fixed(w, h) =
            settings.screen_size
        {
            (w as f64, h as f64)
        } else {
            let (lw, lh) =
                crate::monitor::phys_2_logic((desktop_w as i32, desktop_h as i32), monitor_scale);
            (lw as f64, lh as f64)
        };
        shared::log::info!(
            "enter_rdp: rail={is_rail} fullscreen={is_fullscreen} logical={rdp_w}x{rdp_h} scale={monitor_scale}"
        );

        if is_rail {
            settings.screen_size = rdp_ffi::geom::ScreenSize::Fixed(rdp_w, rdp_h);
            let window = Arc::new(
                el.create_window(
                    winit::window::Window::default_attributes()
                        .with_title("UDS RemoteApp")
                        .with_inner_size(winit::dpi::LogicalSize::new(300.0, 40.0))
                        .with_decorations(false) // Borderless
                        .with_window_icon(Some(crate::logo::load_icon())),
                )?,
            );
            let wid = window.id();
            let renderer = crate::wgpu_render::WgpuRenderer::new(window.clone(), 300, 100)?;
            let rdp_window = RdpWindow {
                window,
                renderer,
                scratch: Vec::new(),
            };
            let server_info = settings.rail.as_ref().and_then(|r| r.server_info.clone());
            let rdp_state = RdpState::new(
                rdp_window,
                settings,
                true,
                coords_scale,
                (rdp_w, rdp_h),
                self.keys_rx.clone(),
                use_rgba,
                tr!("UDS Apps"),
                tr!("Exit"),
            )?;
            self.rdp = Some(Box::new(rdp_state));
            self.register_window(wid, WindowKind::Rdp);

            if let Some(ref srv) = server_info
                && let Some(ref state) = self.rdp
            {
                let cmd_tx = state.command_tx.clone();
                let cmd_ev = state.command_event;
                self.rail_ipc = crate::ipc::bind(&srv.id, &srv.token, move |msg| {
                    let _ = cmd_tx.send(rdp_ffi::messaging::RdpCommand::LaunchRailApp {
                        app: msg.app.clone(),
                        args: msg.args.clone(),
                        dir: msg.working_dir.clone(),
                    });
                    rdp_ffi::Rdp::set_command_event(&cmd_ev);
                })
                .ok();
            }
        } else {
            settings.screen_size = rdp_ffi::geom::ScreenSize::Fixed(rdp_w, rdp_h);
            let window = Arc::new(
                el.create_window(
                    winit::window::Window::default_attributes()
                        .with_title("UDS Remote Desktop")
                        .with_inner_size(winit::dpi::LogicalSize::new(
                            window_logical_w,
                            window_logical_h,
                        ))
                        .with_window_icon(Some(crate::logo::load_icon())),
                )?,
            );
            let wid = window.id();
            let phys = window.inner_size();
            let renderer =
                crate::wgpu_render::WgpuRenderer::new(window.clone(), phys.width, phys.height)?;
            let rdp_window = RdpWindow {
                window,
                renderer,
                scratch: Vec::new(),
            };
            let rdp_state = RdpState::new(
                rdp_window,
                settings,
                false,
                coords_scale,
                desktop_size,
                self.keys_rx.clone(),
                use_rgba,
                tr!("UDS Apps"),
                tr!("Exit"),
            )?;
            if is_fullscreen {
                rdp_state
                    .window
                    .window
                    .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                if let RdpMode::Desktop {
                    ref full_screen, ..
                } = rdp_state.mode
                {
                    full_screen.store(true, Ordering::Relaxed);
                }
            }
            self.rdp = Some(Box::new(rdp_state));
            self.register_window(wid, WindowKind::Rdp);
        }
        while self.keys_rx.try_recv().is_ok() {}
        self.processing_events.store(true, Ordering::Relaxed);
        if let Some(ref state) = self.rdp {
            state.window.window.set_cursor_visible(false);
            state.window.window.request_redraw();
        }

        Ok(())
    }

    pub(crate) fn close_rdp(&mut self) {
        self.processing_events.store(false, Ordering::Relaxed);
        let mut ids = Vec::new();
        if let Some(ref r) = self.rdp {
            ids.push(r.window.window.id());
            if let RdpMode::Rail(ref rail) = r.mode {
                for rw in rail.windows.values() {
                    ids.push(rw.window.id());
                }
            }
        }
        for id in ids {
            self.unregister_window(id);
        }
        self.rdp = None;
        self.rail_ipc = None;
    }

    pub(crate) fn process_rail_actions(&mut self, el: &ActiveEventLoop) {
        let actions = if let Some(ref mut state) = self.rdp
            && let RdpMode::Rail(ref mut rail) = state.mode
        {
            std::mem::take(&mut rail.actions)
        } else {
            return;
        };
        let mut created: Vec<(winit::window::WindowId, u32)> = Vec::new();
        let mut deleted: Vec<winit::window::WindowId> = Vec::new();
        for action in &actions {
            let Some(ref mut state) = self.rdp else { break };
            let RdpMode::Rail(ref mut rail) = state.mode else {
                break;
            };
            match action {                RailAction::Create(id, title, rect, taskbar, decorations, visible, minimized) => {
                    if rail.windows.contains_key(id) {
                        continue;
                    }
                    let Ok(window) = el.create_window(
                        winit::window::Window::default_attributes()
                            .with_title(title.clone())
                            .with_decorations(*decorations)
                            .with_transparent(true)
                            .with_inner_size(winit::dpi::LogicalSize::new(
                                rect.w as f64,
                                rect.h as f64,
                            ))
                            .with_visible(*visible),
                    ) else {
                        continue;
                    };

                    if *minimized {
                        window.set_minimized(true);
                    }

                    let wid = window.id();
                    let sf = state.coords_scale.max(1.0);
                    let (px, py) = crate::monitor::logic_2_phys_pos((rect.x, rect.y), sf);
                    window.set_outer_position(winit::dpi::PhysicalPosition::new(px, py));
                    let window = Arc::new(window);
                    let renderer =
                        crate::wgpu_render::WgpuRenderer::new(window.clone(), rect.w, rect.h).ok();
                    let (rgba_data, pw, ph) =
                        if let Some((w, h, data)) = state.pendings.pixels.remove(id) {
                            (Some(data), w, h)
                        } else {
                            (None, rect.w, rect.h)
                        };
                    if rgba_data.is_some() {
                        window.request_redraw();
                    }
                    rail.windows.insert(
                        *id,
                        RailWindow {
                            id: *id,
                            window: window.clone(),
                            renderer,
                            rgba_data,
                            width: pw,
                            height: ph,
                            rect: *rect,
                            title: String::new(),
                            show_in_taskbar: *taskbar,
                            has_decorations: *decorations,
                            last_focused: false,
                            offscreen: false,
                            rgba_dirty: true,
                        },
                    );

                    // Set active cursor on the new window
                    if state.cursor.visible && !state.cursor.data.is_empty() {
                        let (rgba, w, h, hx, hy) = state.cursor.build_scaled_rgba();
                        if let Ok(source) =
                            winit::window::CustomCursor::from_rgba(rgba, w, h, hx, hy)
                        {
                            let custom_cursor = el.create_custom_cursor(source);
                            window.set_cursor(custom_cursor);
                        }
                    } else {
                        window.set_cursor(winit::window::CursorIcon::Default);
                    }

                    if let Some((rgba, w, h)) = state.pendings.icons.remove(id)
                        && let Ok(icon) = winit::window::Icon::from_rgba(rgba, w, h)
                        && let Some(rw) = rail.windows.get(id)
                    {
                        rw.window.set_window_icon(Some(icon));
                    }

                    created.push((wid, *id));
                    shared::log::debug!("RAIL window created: id={id} {rect:?}");
                }
                RailAction::Delete(id) => {
                    if let Some(rw) = rail.windows.remove(id) {
                        deleted.push(rw.window.id());
                    }
                }
                RailAction::UpdatePosition(id, rect) => {
                    if let Some(rw) = rail.windows.get_mut(id) {
                        shared::log::trace!(
                            "RAIL[{id}] UpdatePosition rect=({},{}) {}x{} button_down={}",
                            rect.x,
                            rect.y,
                            rect.w,
                            rect.h,
                            self.rail_button_down.is_some()
                        );
                        rw.rect = *rect;
                        let _ = rw.window.request_inner_size(winit::dpi::LogicalSize::new(
                            rect.w as f64,
                            rect.h as f64,
                        ));
                        let sf = state.coords_scale.max(1.0);
                        let (px, py) = crate::monitor::logic_2_phys_pos((rect.x, rect.y), sf);
                        rw.window
                            .set_outer_position(winit::dpi::PhysicalPosition::new(px, py));
                    }
                }
                RailAction::SetVisible(id, visible) => {
                    if let Some(rw) = rail.windows.get_mut(id) {
                        rw.window.set_visible(*visible);
                        rw.offscreen = !*visible;
                    }
                }
                RailAction::SetMinimized(id, minimized) => {
                    if let Some(rw) = rail.windows.get_mut(id) {
                        rw.window.set_minimized(*minimized);
                    }
                }
            }
        }
        for (wid, id) in created {
            self.register_window(wid, WindowKind::RdpRail(id));
        }
        for wid in deleted {
            self.unregister_window(wid);
        }
    }

    pub(crate) fn process_rdp_updates(&mut self, el: &ActiveEventLoop) {
        let Some(ref mut state) = self.rdp else {
            return;
        };
        while let Ok(message) = state.update_rx.try_recv() {
            match handle_rdp_message(state, message) {
                RdpActionResult::Continue => {
                    state.window.window.request_redraw();
                }

                RdpActionResult::Disconnect => {
                    self.stop.trigger();
                    self.return_code = crate::types::ReturnCode::Exit;
                    self.close_rdp();
                    el.exit();
                    return;
                }
                RdpActionResult::Error(_) => {
                    self.stop.trigger();
                    self.return_code = crate::types::ReturnCode::Exit;
                    self.close_rdp();
                    el.exit();
                    return;
                }
                _ => {}
            }
        }
        if state.cursor.dirty {
            state.cursor.dirty = false;
            if let RdpMode::Rail(ref mut rail) = state.mode {
                let cursor_opt = if state.cursor.visible && !state.cursor.data.is_empty() {
                    let (rgba, w, h, hx, hy) = state.cursor.build_scaled_rgba();
                    match winit::window::CustomCursor::from_rgba(rgba, w, h, hx, hy) {
                        Ok(source) => Some(el.create_custom_cursor(source)),
                        Err(err) => {
                            log::warn!("Failed to create CustomCursorSource: {:?}", err);
                            None
                        }
                    }
                } else {
                    None
                };

                for rw in rail.windows.values() {
                    if let Some(ref custom_cursor) = cursor_opt {
                        rw.window.set_cursor(custom_cursor.clone());
                    } else {
                        rw.window.set_cursor(winit::window::CursorIcon::Default);
                    }
                }
            }
        }
        while let Ok(raw_key) = state.keys_rx.try_recv() {
            if let Some(sc) = crate::keymap::RdpScanCode::get_from_key(Some(&raw_key.keycode)) {
                let _ = state.command_tx.send(rdp_ffi::messaging::RdpCommand::Input(
                    rdp_ffi::messaging::InputEvent::Keyboard {
                        scancode: sc as u16,
                        pressed: raw_key.pressed,
                        repeat: raw_key.repeat,
                    },
                ));
                if sc == crate::keymap::RdpScanCode::RMenu && !raw_key.pressed {
                    let _ = state.command_tx.send(rdp_ffi::messaging::RdpCommand::Input(
                        rdp_ffi::messaging::InputEvent::Keyboard {
                            scancode: crate::keymap::RdpScanCode::LControl as u16,
                            pressed: false,
                            repeat: false,
                        },
                    ));
                }
                    rdp_ffi::Rdp::set_command_event(&state.command_event);
            }
        }
        if let RdpMode::Desktop { ref mut fps, .. } = state.mode {
            fps.record();
        }
    }
}
