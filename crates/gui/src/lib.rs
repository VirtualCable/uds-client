// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use flume::{Receiver, Sender, bounded};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use shared::{log, system::trigger::Trigger};

pub mod about;
pub mod keymap;
pub mod logo;
mod monitor;
mod popup;
pub mod types;

mod draw;
mod rdp;
mod testing;
mod wgpu_render;

use crate::wgpu_render::WgpuRenderer;
use popup::{PopupKind, PopupState};
use rdp::{RailAction, RailWindow, RdpState, RdpWindow, handle_rdp_message};
use testing::{LauncherInner, TestAction, TestingLauncherState, paint_launcher};
use types::{AppState, GuiMessage, ReturnCode};

#[derive(Debug)]
pub struct RawKey {
    pub keycode: winit::keyboard::KeyCode,
    pub pressed: bool,
    pub repeat: bool,
}

#[derive(Debug)]
enum UserEvent {
    Tick,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowKind {
    Launcher,
    Rdp,
    RdpRail(u32),
    Popup,
    About,
}

pub struct AppHandler {
    launcher: Option<TestingLauncherState>,
    rdp: Option<Box<RdpState>>,
    popup: Option<PopupState>,
    about: Option<crate::about::AboutState>,
    windows: HashMap<WindowId, WindowKind>,

    keys_tx: Sender<RawKey>,
    keys_rx: Receiver<RawKey>,
    gui_messages_rx: Receiver<GuiMessage>,
    processing_events: Arc<AtomicBool>,
    stop: Trigger,
    fps_limit: u32,
    alt_held: bool,
    last_pointer: Option<winit::dpi::PhysicalPosition<f64>>,
    rail_button_down: Option<u32>,
    return_code: ReturnCode,
    initial_state: Option<AppState>,
    first_resume: bool,
    proxy: EventLoopProxy<UserEvent>,
}

pub fn run_gui(
    _catalog: gettext::Catalog,
    initial_state: Option<AppState>,
    messages_rx: Receiver<GuiMessage>,
    stop: Trigger,
    fps_limit: Option<u32>,
) -> Result<ReturnCode> {
    let (keys_tx, keys_rx) = bounded::<RawKey>(1024);
    let processing_events = Arc::new(AtomicBool::new(false));
    let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();

    let mut app = AppHandler {
        launcher: None,
        rdp: None,
        popup: None,
        about: None,
        windows: HashMap::new(),
        keys_tx,
        keys_rx,
        gui_messages_rx: messages_rx,
        processing_events,
        stop,
        fps_limit: fps_limit.unwrap_or(60),
        alt_held: false,
        last_pointer: None,
        rail_button_down: None,
        return_code: ReturnCode::Exit,
        initial_state,
        first_resume: true,
        proxy,
    };
    event_loop.run_app(&mut app)?;
    Ok(app.return_code)
}

impl AppHandler {
    fn register_window(&mut self, wid: WindowId, kind: WindowKind) {
        self.windows.insert(wid, kind);
    }
    fn unregister_window(&mut self, wid: WindowId) {
        self.windows.remove(&wid);
    }

    fn open_launcher(&mut self, el: &ActiveEventLoop, inner: LauncherInner) -> Result<()> {
        let window = Arc::new(
            el.create_window(
                Window::default_attributes()
                    .with_title("UDS Launcher")
                    .with_inner_size(winit::dpi::LogicalSize::new(400.0, 300.0))
                    .with_window_icon(Some(logo::load_icon()))
                    .with_resizable(false),
            )?,
        );
        let wid = window.id();
        let phys = window.inner_size();
        let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;
        self.launcher = Some(TestingLauncherState {
            window: Some(window),
            renderer: Some(renderer),
            inner,
            last_mouse_pos: None,
        });
        self.register_window(wid, WindowKind::Launcher);
        Ok(())
    }

    fn close_launcher(&mut self) {
        if let Some(ref l) = self.launcher
            && let Some(w) = &l.window
        {
            self.unregister_window(w.id());
        }
        self.launcher = None;
    }

    fn open_rdp(
        &mut self,
        el: &ActiveEventLoop,
        mut settings: rdp_ffi::settings::RdpSettings,
    ) -> Result<()> {
        let is_rail = settings.rail_app.is_some();
        let use_rgba = cfg!(target_os = "macos");

        let monitor_scale = monitor::scale(0);
        let (desktop_w, desktop_h) = monitor::size(0).unwrap_or((1920, 1080));
        let use_local_scaler = settings.use_local_scaler;
        let local_scale = if use_local_scaler { monitor_scale } else { 1.0 };

        // If full screen size or rail, use full monitor size as RDP desktop, otherwise use fixed size or monitor size as specified
        let (rdp_w, rdp_h) = match (settings.screen_size, is_rail) {
            (rdp_ffi::geom::ScreenSize::Full, _) | (_, true) => {
                let (lw, lh) =
                    monitor::phys_2_logic((desktop_w as i32, desktop_h as i32), local_scale);
                (lw as u32, lh as u32)
            }
            (rdp_ffi::geom::ScreenSize::Fixed(w, h), _) => (w, h),
        };
        let coords_scale = if use_local_scaler {
            settings.scale_factor = 1.0;
            monitor_scale
        } else {
            settings.scale_factor = monitor_scale;
            1.0
        };
        let desktop_size = (rdp_w, rdp_h);
        let is_fullscreen = settings.screen_size.is_fullscreen() && !is_rail;
        let (window_logical_w, window_logical_h) =
            monitor::phys_2_logic((desktop_w as i32, desktop_h as i32), monitor_scale);
        log::info!(
            "enter_rdp: rail={is_rail} fullscreen={is_fullscreen} logical={rdp_w}x{rdp_h} scale={monitor_scale}"
        );

        if is_rail {
            // Screen size shoud be the real one for rail, with all consecuences
            settings.screen_size = rdp_ffi::geom::ScreenSize::Fixed(rdp_w, rdp_h);
            let window = Arc::new(
                el.create_window(
                    Window::default_attributes()
                        .with_title("UDS RemoteApp")
                        .with_inner_size(winit::dpi::LogicalSize::new(300.0, 100.0))
                        .with_window_icon(Some(logo::load_icon())),
                )?,
            );
            let wid = window.id();
            let renderer = WgpuRenderer::new(window.clone(), 300, 100)?;
            let rdp_window = RdpWindow {
                window,
                renderer,
                scratch: Vec::new(),
            };
            let rdp_state = RdpState::new(
                rdp_window,
                settings,
                true,
                coords_scale,
                (rdp_w, rdp_h),
                self.keys_rx.clone(),
                use_rgba,
            )?;
            self.rdp = Some(Box::new(rdp_state));
            self.register_window(wid, WindowKind::Rdp);
        } else {
            settings.screen_size = rdp_ffi::geom::ScreenSize::Fixed(rdp_w, rdp_h);
            let window = Arc::new(
                el.create_window(
                    Window::default_attributes()
                        .with_title("UDS Remote Desktop")
                        .with_inner_size(winit::dpi::LogicalSize::new(
                            window_logical_w,
                            window_logical_h,
                        ))
                        .with_window_icon(Some(logo::load_icon())),
                )?,
            );
            let wid = window.id();
            let phys = window.inner_size();
            let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;
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
            )?;
            if is_fullscreen {
                rdp_state
                    .window
                    .window
                    .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                rdp_state.full_screen.store(true, Ordering::Relaxed);
            }
            self.rdp = Some(Box::new(rdp_state));
            self.register_window(wid, WindowKind::Rdp);
        }
        while self.keys_rx.try_recv().is_ok() {}
        self.processing_events.store(true, Ordering::Relaxed);
        if let Some(ref state) = self.rdp {
            state.window.window.set_cursor_visible(false);
        }

        // Spawn frame pacing thread
        let proxy = self.proxy.clone();
        let fps = self.fps_limit;
        let stop = self.stop.clone();
        std::thread::spawn(move || {
            let interval = Duration::from_secs_f64(1.0 / fps as f64);
            while !stop.is_triggered() {
                std::thread::sleep(interval);
                let _ = proxy.send_event(UserEvent::Tick);
            }
        });
        Ok(())
    }

    fn close_rdp(&mut self) {
        self.processing_events.store(false, Ordering::Relaxed);
        if let Some(ref r) = self.rdp {
            self.unregister_window(r.window.window.id());
        }
        self.rdp = None;
    }
}

impl AppHandler {
    fn handle_keyboard(&mut self, el: &ActiveEventLoop, event: &WindowEvent) -> bool {
        let WindowEvent::KeyboardInput { event: key_ev, .. } = event else {
            return false;
        };
        let PhysicalKey::Code(code) = key_ev.physical_key else {
            return false;
        };

        // Track Alt
        match code {
            winit::keyboard::KeyCode::AltLeft | winit::keyboard::KeyCode::AltRight => {
                self.alt_held = key_ev.state.is_pressed();
            }
            _ => {}
        }

        if !self.processing_events.load(Ordering::Relaxed) {
            return false;
        }

        // Hotkeys
        if self.alt_held && key_ev.state.is_pressed() && !key_ev.repeat {
            let is_rail = self.rdp.as_ref().is_some_and(|s| s.is_rail);
            match code {
                winit::keyboard::KeyCode::Enter if !is_rail => {
                    log::debug!("Alt+Enter → fullscreen");
                    self.toggle_fullscreen();
                    return true;
                }
                winit::keyboard::KeyCode::KeyF if !is_rail => {
                    if let Some(ref s) = self.rdp {
                        s.fps.toggle();
                    }
                    return true;
                }
                winit::keyboard::KeyCode::F4 => {
                    log::debug!("Alt+F4 → exit");
                    self.stop.trigger();
                    el.exit();
                    return true;
                }
                _ => {}
            }
        }

        let raw = RawKey {
            keycode: code,
            pressed: key_ev.state.is_pressed(),
            repeat: key_ev.repeat,
        };
        let _ = self.keys_tx.send(raw);
        true
    }

    fn toggle_fullscreen(&mut self) {
        let Some(s) = &mut self.rdp else { return };
        let is_fs = s.full_screen.load(Ordering::Relaxed);
        if !is_fs {
            s.window
                .window
                .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            s.full_screen.store(true, Ordering::Relaxed);
        } else {
            s.window.window.set_fullscreen(None);
            s.full_screen.store(false, Ordering::Relaxed);
            if s.last_windowed_size.is_none() {
                let phys = s.window.window.inner_size();
                let w = (phys.width as f64 * 2.0 / 3.0) as u32;
                let h = (phys.height as f64 * 2.0 / 3.0) as u32;
                let sf = s.window.window.scale_factor();
                let _ = s
                    .window
                    .window
                    .request_inner_size(winit::dpi::LogicalSize::new(w as f64 / sf, h as f64 / sf));
                s.last_windowed_size = Some((w, h));
            }
        }
    }
}

impl AppHandler {
    fn handle_launcher_event(&mut self, el: &ActiveEventLoop, event: WindowEvent) {
        let Some(ref mut l) = self.launcher else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => {
                self.stop.trigger();
                el.exit();
            }
            WindowEvent::RedrawRequested => {
                paint_launcher(l);
            }
            WindowEvent::MouseInput { state, button, .. }
                if state.is_pressed() && button == winit::event::MouseButton::Left =>
            {
                if let Some(pos) = l.last_mouse_pos {
                    l.inner.handle_click(pos.0, pos.1);
                }
                if let Some(w) = &l.window {
                    w.request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let sf = *monitor::SCALE_FACTOR as f32;
                l.last_mouse_pos = Some((position.x as f32 / sf, position.y as f32 / sf));
            }
            _ => {}
        }
    }

    fn handle_rdp_input(&mut self, event: &WindowEvent) -> bool {
        let Some(s) = &mut self.rdp else { return true };
        if s.is_rail {
            match event {
                WindowEvent::CloseRequested => return false,
                _ => return true,
            }
        }
        match event {
            WindowEvent::CloseRequested => return false,
            WindowEvent::Resized(_) => {
                s.request_screen_resize();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_pointer = Some(*position);
                s.cursor.x = position.x as f32;
                s.cursor.y = position.y as f32;
                s.window.window.request_redraw();
                let gdi_w = unsafe { (*s.gdi).width as u32 };
                let gdi_h = unsafe { (*s.gdi).height as u32 };
                let phys_w = s.window.window.inner_size().width;
                let phys_h = s.window.window.inner_size().height;
                let x = ((position.x * gdi_w as f64) / phys_w as f64)
                    .round()
                    .clamp(0.0, (gdi_w - 1) as f64) as u16;
                let y = ((position.y * gdi_h as f64) / phys_h as f64)
                    .round()
                    .clamp(0.0, (gdi_h - 1) as f64) as u16;
                let _ = s.command_tx.send(rdp_ffi::commands::RdpCommand::Input(
                    rdp_ffi::commands::InputEvent::Mouse {
                        flags: rdp_ffi::sys::PTR_FLAGS_MOVE as u16,
                        x,
                        y,
                    },
                ));
                unsafe {
                    rdp_ffi::sys::SetEvent(s.command_event.as_handle());
                }
                // Pinbar
                let is_fs = s.full_screen.load(Ordering::Relaxed);
                if position.y < 5.0
                    && position.x > std::cmp::max(phys_w, 1) as f64 * 0.4
                    && position.x < phys_w as f64 * 0.6
                {
                    s.pinbar.visible = is_fs;
                }
                if position.y > 32.0 {
                    s.pinbar.visible = false;
                }
            }
            WindowEvent::MouseInput {
                state: btn, button, ..
            } => {
                // Pinbar click — only on press
                if btn.is_pressed()
                    && let Some(pos) = self.last_pointer
                    && s.pinbar.visible
                    && *button == winit::event::MouseButton::Left
                {
                    let px = pos.x as f32;
                    if s.pinbar.btn_fs_x.contains(&px) {
                        self.toggle_fullscreen();
                        return true;
                    }
                    if s.pinbar.btn_close_x.contains(&px) {
                        return false;
                    }
                }

                if let Some(pos) = self.last_pointer {
                    let gdi_w = unsafe { (*s.gdi).width as u32 };
                    let gdi_h = unsafe { (*s.gdi).height as u32 };
                    let phys_w = s.window.window.inner_size().width;
                    let phys_h = s.window.window.inner_size().height;
                    let x = ((pos.x * gdi_w as f64) / phys_w as f64)
                        .round()
                        .clamp(0.0, (gdi_w - 1) as f64) as u16;
                    let y = ((pos.y * gdi_h as f64) / phys_h as f64)
                        .round()
                        .clamp(0.0, (gdi_h - 1) as f64) as u16;
                    let flags = match *button {
                        winit::event::MouseButton::Left => rdp_ffi::sys::PTR_FLAGS_BUTTON1,
                        winit::event::MouseButton::Right => rdp_ffi::sys::PTR_FLAGS_BUTTON2,
                        winit::event::MouseButton::Middle => rdp_ffi::sys::PTR_FLAGS_BUTTON3,
                        _ => 0,
                    } as u16;
                    if flags != 0 {
                        let f = flags
                            | if btn.is_pressed() {
                                rdp_ffi::sys::PTR_FLAGS_DOWN as u16
                            } else {
                                0
                            };
                        let _ = s.command_tx.send(rdp_ffi::commands::RdpCommand::Input(
                            rdp_ffi::commands::InputEvent::Mouse { flags: f, x, y },
                        ));
                        unsafe {
                            rdp_ffi::sys::SetEvent(s.command_event.as_handle());
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y as i32,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as i32,
                };
                let mut wd = (dy as f32 * 120.0) as i32;
                let flags = (rdp_ffi::sys::PTR_FLAGS_WHEEL as u16)
                    | if wd < 0 {
                        wd = -wd;
                        rdp_ffi::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16
                    } else {
                        0
                    };
                while wd > 0 {
                    let step: u16 = if wd > 0xFF { 0xFF } else { (wd & 0xFF) as u16 };
                    wd -= step as i32;
                    let cflags = if flags & (rdp_ffi::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16) != 0 {
                        flags | (0x100 - step)
                    } else {
                        flags | step
                    };
                    let _ = s.command_tx.send(rdp_ffi::commands::RdpCommand::Input(
                        rdp_ffi::commands::InputEvent::Mouse {
                            flags: cflags,
                            x: 0,
                            y: 0,
                        },
                    ));
                    unsafe {
                        rdp_ffi::sys::SetEvent(s.command_event.as_handle());
                    }
                }
            }
            _ => {}
        }
        true
    }

    fn handle_popup_event(&mut self, event: WindowEvent) {
        if self.popup.is_none() {
            return;
        }
        let popup = self.popup.as_mut().unwrap();
        let mut close = false;
        match event {
            WindowEvent::CloseRequested => close = true,
            WindowEvent::RedrawRequested => {
                popup.paint();
            }
            WindowEvent::MouseInput { state, button, .. }
                if state.is_pressed() && button == winit::event::MouseButton::Left =>
            {
                if let Some(pos) = self.last_pointer {
                    close = popup.handle_click(pos.x as f32, pos.y as f32);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_pointer = Some(position);
            }
            _ => {}
        }
        if close {
            let wid = popup.window.id();
            self.unregister_window(wid);
            self.popup = None;
        }
    }

    fn handle_about_event(&mut self, event: WindowEvent) {
        let Some(ref mut a) = self.about else { return };
        match event {
            WindowEvent::CloseRequested => {
                self.about = None;
            }
            WindowEvent::MouseInput { state, .. } if state.is_pressed() => {
                self.about = None;
            }
            WindowEvent::RedrawRequested => {
                a.paint();
            }
            _ => {}
        }
    }
}

// ── Message processing ────────────────────────────────────

impl AppHandler {
    fn handle_rail_redraw(&mut self, rail_id: u32) {
        if let Some(ref mut state) = self.rdp
            && let Some(rw) = state.rail_windows.get_mut(&rail_id)
        {
            // Force position every frame to prevent Windows cascading offset
            let sf = state.coords_scale.max(1.0);
            let (px, py) = monitor::logic_2_phys_pos((rw.rect.x, rw.rect.y), sf);
            rw.window
                .set_outer_position(winit::dpi::PhysicalPosition::new(px, py));
            if let (Some(rgba), Some(ref mut renderer)) = (&rw.rgba_data, rw.renderer.as_mut()) {
                renderer.update_and_render(rgba.as_slice(), rw.width, rw.height, &[], &[], None);
            }
        }
    }

    fn handle_rail_event(&mut self, rail_id: u32, event: WindowEvent) {
        let Some(ref mut state) = self.rdp else {
            return;
        };
        let Some(rail_channel) = state.rail_channel.clone() else {
            return;
        };
        let cmd_tx = state.command_tx.clone();
        let cmd_ev = state.command_event;

        if let WindowEvent::MouseInput { state: btn, .. } = &event {
            self.rail_button_down = if btn.is_pressed() {
                Some(rail_id)
            } else {
                None
            };
        }

        match event {
            WindowEvent::CloseRequested => {
                rail_channel.send_system_command(rail_id, rdp_ffi::consts::SC_CLOSE as u16);
            }
            WindowEvent::Focused(focused) => {
                if let Some(rw) = state.rail_windows.get_mut(&rail_id) {
                    if focused && !rw.last_focused && rw.show_in_taskbar {
                        rail_channel.send_activate(rail_id, true);
                    }
                    rw.last_focused = focused;
                }
            }
            WindowEvent::CursorMoved { ref position, .. } => {
                self.last_pointer = Some(*position);
                if let Some(rw) = state.rail_windows.get(&rail_id) {
                    let sf = state.coords_scale;
                    let (gx, gy) = monitor::phys_2_logic(
                        (
                            (position.x + rw.rect.x as f64 * sf).round() as i32,
                            (position.y + rw.rect.y as f64 * sf).round() as i32,
                        ),
                        sf,
                    );
                    let dw = state.desktop_size.0.saturating_sub(1) as i32;
                    let dh = state.desktop_size.1.saturating_sub(1) as i32;
                    let gx = gx.clamp(0, dw) as u16;
                    let gy = gy.clamp(0, dh) as u16;
                    shared::log::debug!(
                        "RAIL[{rail_id}] MoveSend pos=({:.0},{:.0}) sf={sf} rect=({},{})+{}x{} → g=({gx},{gy}) clamp=({dw:.0},{dh:.0})",
                        position.x, position.y,
                        rw.rect.x, rw.rect.y, rw.rect.w, rw.rect.h,
                    );
                    let _ = cmd_tx.send(rdp_ffi::commands::RdpCommand::Input(
                        rdp_ffi::commands::InputEvent::Mouse {
                            flags: rdp_ffi::sys::PTR_FLAGS_MOVE as u16,
                            x: gx,
                            y: gy,
                        },
                    ));
                    unsafe {
                        rdp_ffi::sys::SetEvent(cmd_ev.as_handle());
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                // Do NOT synthesize release — Windows SetCapture will deliver
                // the real MouseInput release even when cursor is outside.
                shared::log::debug!(
                    "RAIL[{rail_id}] CursorLeft (button_down={})",
                    self.rail_button_down.is_some()
                );
            }
            WindowEvent::MouseInput {
                button, state: btn, ..
            } => {
                shared::log::debug!(
                    "RAIL[{rail_id}] MouseInput({button:?} pressed={})",
                    btn.is_pressed()
                );
                if btn.is_pressed()
                    && state
                        .rail_windows
                        .get(&rail_id)
                        .is_some_and(|rw| rw.show_in_taskbar)
                {
                    rail_channel.send_activate(rail_id, true);
                }
                if let Some(pos) = self.last_pointer
                    && let Some(rw) = state.rail_windows.get(&rail_id)
                {
                    let bm = match button {
                        winit::event::MouseButton::Left => rdp_ffi::sys::PTR_FLAGS_BUTTON1,
                        winit::event::MouseButton::Right => rdp_ffi::sys::PTR_FLAGS_BUTTON2,
                        winit::event::MouseButton::Middle => rdp_ffi::sys::PTR_FLAGS_BUTTON3,
                        _ => return,
                    } as u16;
                    let f = bm
                        | if btn.is_pressed() {
                            rdp_ffi::sys::PTR_FLAGS_DOWN as u16
                        } else {
                            0
                        };
                    let sf = state.coords_scale;
                    let (gx, gy) = monitor::phys_2_logic(
                        (
                            (pos.x + rw.rect.x as f64 * sf).round() as i32,
                            (pos.y + rw.rect.y as f64 * sf).round() as i32,
                        ),
                        sf,
                    );
                    let dw = state.desktop_size.0.saturating_sub(1) as i32;
                    let dh = state.desktop_size.1.saturating_sub(1) as i32;
                    let gx = gx.clamp(0, dw) as u16;
                    let gy = gy.clamp(0, dh) as u16;
                    log::debug!(
                        "RAIL[{rail_id}] MouseClick → flags={f} x={gx} y={gy} (phys=({:.0},{:.0}) sf={sf} rect=({},{})+{}x{})",
                        pos.x,
                        pos.y,
                        rw.rect.x,
                        rw.rect.y,
                        rw.rect.w,
                        rw.rect.h
                    );
                    let _ = cmd_tx.send(rdp_ffi::commands::RdpCommand::Input(
                        rdp_ffi::commands::InputEvent::Mouse {
                            flags: f,
                            x: gx,
                            y: gy,
                        },
                    ));
                    unsafe {
                        rdp_ffi::sys::SetEvent(cmd_ev.as_handle());
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y as i32,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as i32,
                };
                let mut wd = (dy as f32 * 120.0) as i32;
                let flags = (rdp_ffi::sys::PTR_FLAGS_WHEEL as u16)
                    | if wd < 0 {
                        wd = -wd;
                        rdp_ffi::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16
                    } else {
                        0
                    };
                while wd > 0 {
                    let step: u16 = if wd > 0xFF { 0xFF } else { (wd & 0xFF) as u16 };
                    wd -= step as i32;
                    let cflags = if flags & (rdp_ffi::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16) != 0 {
                        flags | (0x100 - step)
                    } else {
                        flags | step
                    };
                    let _ = cmd_tx.send(rdp_ffi::commands::RdpCommand::Input(
                        rdp_ffi::commands::InputEvent::Mouse {
                            flags: cflags,
                            x: 0,
                            y: 0,
                        },
                    ));
                    unsafe {
                        rdp_ffi::sys::SetEvent(cmd_ev.as_handle());
                    }
                }
            }
            _ => {}
        }
    }

    fn process_rail_actions(&mut self, el: &ActiveEventLoop) {
        let actions = if let Some(ref mut state) = self.rdp {
            std::mem::take(&mut state.rail_actions)
        } else {
            return;
        };
        let mut regs: Vec<(winit::window::WindowId, u32)> = Vec::new();
        for action in &actions {
            let Some(ref mut state) = self.rdp else { break };
            match action {
                RailAction::Create(id, title, rect, taskbar, decorations, visible) => {
                    if state.rail_windows.contains_key(id) {
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
                    let wid = window.id();
                    // Position window at server-specified coordinates
                    let sf = state.coords_scale.max(1.0);
                    let (px, py) = monitor::logic_2_phys_pos((rect.x, rect.y), sf);
                    window.set_outer_position(winit::dpi::PhysicalPosition::new(px, py));
                    let window = Arc::new(window);
                    let renderer =
                        crate::wgpu_render::WgpuRenderer::new(window.clone(), rect.w, rect.h).ok();
                    let (rgba_data, pw, ph) =
                        if let Some((w, h, data)) = state.pending_pixels.remove(id) {
                            (Some(data), w, h)
                        } else {
                            (None, rect.w, rect.h)
                        };
                    if rgba_data.is_some() {
                        window.request_redraw();
                    }
                    state.rail_windows.insert(
                        *id,
                        RailWindow {
                            id: *id,
                            window,
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
                        },
                    );
                    regs.push((wid, *id));
                    log::debug!("RAIL window created: id={id} {rect:?}");
                }
                RailAction::Delete(id) => {
                    if let Some(rw) = state.rail_windows.remove(id) {
                        regs.push((rw.window.id(), *id));
                    }
                }
                RailAction::UpdatePosition(id, rect) => {
                    if let Some(rw) = state.rail_windows.get_mut(id) {
                        shared::log::debug!(
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
                        let (px, py) = monitor::logic_2_phys_pos((rect.x, rect.y), sf);
                        rw.window
                            .set_outer_position(winit::dpi::PhysicalPosition::new(px, py));
                    }
                }
                RailAction::SetVisible(id, visible) => {
                    if let Some(rw) = state.rail_windows.get_mut(id) {
                        rw.window.set_visible(*visible);
                        rw.offscreen = !*visible;
                    }
                }
            }
        }
        for (wid, id) in regs {
            self.register_window(wid, WindowKind::RdpRail(id));
        }
    }

    fn process_gui_messages(&mut self, el: &ActiveEventLoop) {
        while let Ok(msg) = self.gui_messages_rx.try_recv() {
            match msg {
                GuiMessage::Close => {
                    self.stop.trigger();
                    el.exit();
                    return;
                }
                GuiMessage::Hide => {
                    if let Some(ref mut l) = self.launcher {
                        l.inner = LauncherInner::Invisible;
                        if let Some(w) = &l.window {
                            w.set_visible(false);
                        }
                    }
                }
                GuiMessage::ShowError(err) => {
                    if let Ok(p) = PopupState::new(el, PopupKind::Error(err)) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Popup);
                        self.popup = Some(p);
                    }
                }
                GuiMessage::ShowWarning(msg) => {
                    if let Ok(p) = PopupState::new(el, PopupKind::Warning(msg)) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Popup);
                        self.popup = Some(p);
                    }
                }
                GuiMessage::ShowYesNo(msg, resp) => {
                    if let Ok(p) = PopupState::new(
                        el,
                        PopupKind::YesNo {
                            message: msg,
                            response: resp,
                        },
                    ) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Popup);
                        self.popup = Some(p);
                    }
                }
                GuiMessage::ShowProgress => {
                    if let Some(ref mut l) = self.launcher {
                        l.inner = LauncherInner::Progress {
                            pct: 0,
                            message: String::new(),
                        };
                        if let Some(w) = &l.window {
                            w.set_visible(true);
                            w.request_redraw();
                        }
                    }
                }
                GuiMessage::Progress(pct, msg) => {
                    if let Some(ref mut l) = self.launcher {
                        l.inner = LauncherInner::Progress { pct, message: msg };
                        if let Some(w) = &l.window {
                            w.request_redraw();
                        }
                    }
                }
                GuiMessage::ConnectRdp(settings) => {
                    self.close_launcher();
                    if let Err(e) = self.open_rdp(el, settings) {
                        log::error!("Failed to enter RDP: {e}");
                        self.stop.trigger();
                        el.exit();
                        return;
                    }
                }
            }
        }

        // Launcher test actions
        if let Some(ref mut launcher) = self.launcher
            && let Some(action) = launcher.inner.take_request()
        {
            match action {
                TestAction::ShowProgress => {
                    launcher.inner = LauncherInner::Progress {
                        pct: 0,
                        message: String::new(),
                    }
                }
                TestAction::GoInvisible => launcher.inner = LauncherInner::Invisible,
                TestAction::ShowWarning => {
                    launcher.inner = LauncherInner::Warning("This is a warning message.".into())
                }
                TestAction::ShowError => {
                    launcher.inner = LauncherInner::Error("This is an error message.".into())
                }
                TestAction::ShowYesNo => {
                    let (rtx, _) = tokio::sync::oneshot::channel::<bool>();
                    launcher.inner = LauncherInner::YesNo {
                        message: "Do you want to continue?".into(),
                        response: Arc::new(std::sync::RwLock::new(Some(rtx))),
                    };
                }
                TestAction::ConnectRdp
                | TestAction::ConnectRdpPreconnection
                | TestAction::ConnectRail => {
                    let is_rail = matches!(action, TestAction::ConnectRail);
                    let settings = rdp_ffi::settings::RdpSettings {
                        server: "172.27.247.161".to_string(),
                        user: "user".to_string(),
                        password: "temporal".to_string(),
                        screen_size: rdp_ffi::geom::ScreenSize::Full,
                        rail_app: if is_rail {
                            //Some("c:\\windows\\notepad.exe".to_string())
                            Some("c:\\windows\\system32\\mspaint.exe".to_string())
                        } else {
                            None
                        },
                        best_experience: true,
                        use_local_scaler: true,
                        ..Default::default()
                    };
                    self.close_launcher();
                    if let Err(e) = self.open_rdp(el, settings) {
                        log::error!("Failed to enter RDP: {e}");
                        self.stop.trigger();
                        el.exit();
                    }
                }
            }
        }
    }

    fn process_rdp_updates(&mut self, el: &ActiveEventLoop) {
        let Some(ref mut state) = self.rdp else {
            return;
        };
        while let Ok(message) = state.update_rx.try_recv() {
            match handle_rdp_message(state, message) {
                rdp::RdpActionResult::Continue if !state.is_rail => {
                    state.window.window.request_redraw();
                }

                rdp::RdpActionResult::Disconnect => {
                    self.stop.trigger();
                    self.return_code = ReturnCode::Exit;
                    self.close_rdp();
                    el.exit();
                    return;
                }
                rdp::RdpActionResult::Error(_) => {
                    self.stop.trigger();
                    self.return_code = ReturnCode::Exit;
                    self.close_rdp();
                    el.exit();
                    return;
                }
                _ => {}
            }
        }
        while let Ok(raw_key) = state.keys_rx.try_recv() {
            if let Some(sc) = keymap::RdpScanCode::get_from_key(Some(&raw_key.keycode)) {
                let _ = state.command_tx.send(rdp_ffi::commands::RdpCommand::Input(
                    rdp_ffi::commands::InputEvent::Keyboard {
                        scancode: sc as u16,
                        pressed: raw_key.pressed,
                    },
                ));
                unsafe {
                    rdp_ffi::sys::SetEvent(state.command_event.as_handle());
                }
            }
        }
        state.fps.record();
    }
}

// ── ApplicationHandler ────────────────────────────────────

impl ApplicationHandler<UserEvent> for AppHandler {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        monitor::populate(el);
        if self.first_resume {
            self.first_resume = false;
            let inner = match self.initial_state.take().unwrap_or_default() {
                AppState::Test => LauncherInner::new_test(),
                AppState::Invisible => LauncherInner::Invisible,
            };
            let _ = self.open_launcher(el, inner);
        }
        el.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(16),
        ));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, wid: WindowId, event: WindowEvent) {
        // Keyboard — global, not tied to a specific window
        if matches!(&event, WindowEvent::KeyboardInput { .. }) && self.handle_keyboard(el, &event) {
            return;
        }

        match event {
            WindowEvent::RedrawRequested => {
                // Process messages and updates (always)
                self.process_gui_messages(el);
                self.process_rdp_updates(el);
                self.process_rail_actions(el);

                // Dispatch redraw by window kind
                match self.windows.get(&wid) {
                    Some(WindowKind::Launcher) => {
                        self.handle_launcher_event(el, WindowEvent::RedrawRequested)
                    }
                    Some(WindowKind::Rdp) if !self.rdp.as_ref().is_some_and(|s| s.is_rail) => {
                        let _ = self.rdp.as_mut().map(|s| s.update_screen());
                    }
                    Some(&WindowKind::RdpRail(id)) => {
                        self.handle_rail_redraw(id);
                    }
                    Some(WindowKind::About) => {
                        self.handle_about_event(WindowEvent::RedrawRequested)
                    }
                    _ => {}
                }
            }
            _ => {
                // Dispatch by window kind
                match self.windows.get(&wid) {
                    Some(WindowKind::Launcher) => self.handle_launcher_event(el, event),
                    #[allow(clippy::collapsible_match)]
                    Some(WindowKind::Rdp) => {
                        if !self.handle_rdp_input(&event) {
                            self.stop.trigger();
                            el.exit();
                        }
                    }
                    Some(&WindowKind::RdpRail(id)) => {
                        self.handle_rail_event(id, event);
                    }
                    Some(WindowKind::Popup) => self.handle_popup_event(event),
                    Some(WindowKind::About) => self.handle_about_event(event),
                    _ => {}
                }
            }
        }
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Tick => {
                if let Some(ref r) = self.rdp {
                    r.window.window.request_redraw();
                }
                if let Some(ref p) = self.popup {
                    p.window.request_redraw();
                }
            }
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {}

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.stop.trigger();
    }
}
