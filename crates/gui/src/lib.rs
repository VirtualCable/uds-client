// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use flume::{Receiver, Sender, bounded};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use shared::{log, system::trigger::Trigger};

pub mod about;
pub mod keymap;
pub mod logo;
mod monitor;
mod popup;
pub mod types;
pub mod window;

mod draw;
mod launcher;
mod session;
mod wgpu_render;

use crate::wgpu_render::WgpuRenderer;
use launcher::{LauncherInner, LauncherState, TestAction, paint_launcher};
use popup::{PopupKind, PopupState};
use session::{RdpState, RdpWindow, handle_rdp_message};
use types::{AppState, GuiMessage, ReturnCode};

#[derive(Debug)]
pub struct RawKey {
    pub keycode: winit::keyboard::KeyCode,
    pub pressed: bool,
    pub repeat: bool,
}

pub struct AppHandler {
    // Windows — at most one of each type active
    launcher: Option<LauncherState>,
    rdp: Option<Box<RdpState>>,
    popup: Option<PopupState>,
    about: Option<crate::about::AboutState>,

    // Channels
    keys_tx: Sender<RawKey>,
    keys_rx: Receiver<RawKey>,
    gui_messages_rx: Receiver<GuiMessage>,
    processing_events: Arc<AtomicBool>,
    stop: Trigger,
    fps_limit: Option<u32>,
    alt_held: bool,
    last_pointer: Option<winit::dpi::PhysicalPosition<f64>>,
    return_code: ReturnCode,
    initial_state: Option<AppState>,
    first_resume: bool,
}

pub fn run_gui(
    _catalog: gettext::Catalog,
    initial_state: Option<AppState>,
    messages_rx: Receiver<GuiMessage>,
    stop: Trigger,
    _fps_limit: Option<u32>,
) -> Result<ReturnCode> {
    let (keys_tx, keys_rx) = bounded::<RawKey>(1024);
    let processing_events = Arc::new(AtomicBool::new(false));
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = AppHandler {
        launcher: None,
        rdp: None,
        popup: None,
        about: None,
        keys_tx,
        keys_rx,
        gui_messages_rx: messages_rx,
        processing_events,
        stop,
        fps_limit: _fps_limit,
        alt_held: false,
        last_pointer: None,
        return_code: ReturnCode::Exit,
        initial_state,
        first_resume: true,
    };
    event_loop.run_app(&mut app)?;
    Ok(app.return_code)
}

impl AppHandler {
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
        let phys = window.inner_size();
        let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;
        self.launcher = Some(LauncherState {
            window: Some(window),
            renderer: Some(renderer),
            inner,
            last_mouse_pos: None,
        });
        Ok(())
    }

    fn close_launcher(&mut self) {
        self.launcher = None;
    }

    fn open_rdp(
        &mut self,
        el: &ActiveEventLoop,
        mut settings: rdp::settings::RdpSettings,
    ) -> Result<()> {
        let is_rail = settings.rail_app.is_some();
        let use_rgba = cfg!(target_os = "macos");
        let (desktop_w, desktop_h) = monitor::size(0).unwrap_or((1920, 1080));
        let monitor_scale = monitor::scale(0);
        let (rdp_w, rdp_h) = match settings.screen_size {
            rdp::geom::ScreenSize::Full => {
                let (lw, lh) =
                    monitor::phys_2_logic((desktop_w as i32, desktop_h as i32), monitor_scale);
                (lw as u32, lh as u32)
            }
            rdp::geom::ScreenSize::Fixed(w, h) => (w, h),
        };
        settings.scale_factor = monitor_scale;
        let is_fullscreen = settings.screen_size.is_fullscreen() && !is_rail;
        let (window_logical_w, window_logical_h) =
            monitor::phys_2_logic((desktop_w as i32, desktop_h as i32), monitor_scale);
        log::info!(
            "enter_rdp: rail={is_rail} fullscreen={is_fullscreen} logical={rdp_w}x{rdp_h} scale={monitor_scale} desktop={desktop_w}x{desktop_h}"
        );

        if is_rail {
            settings.screen_size = rdp::geom::ScreenSize::Fixed(rdp_w, rdp_h);
            let window = Arc::new(
                el.create_window(
                    Window::default_attributes()
                        .with_title("UDS RemoteApp")
                        .with_inner_size(winit::dpi::LogicalSize::new(300.0, 100.0))
                        .with_window_icon(Some(logo::load_icon())),
                )?,
            );
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
                monitor_scale,
                (rdp_w, rdp_h),
                self.keys_rx.clone(),
                use_rgba,
            )?;
            self.rdp = Some(Box::new(rdp_state));
        } else {
            settings.screen_size = rdp::geom::ScreenSize::Fixed(rdp_w, rdp_h);
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
                monitor_scale,
                (rdp_w, rdp_h),
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
        }
        while self.keys_rx.try_recv().is_ok() {}
        self.processing_events.store(true, Ordering::Relaxed);
        if let Some(ref state) = self.rdp {
            state.window.window.set_cursor_visible(false);
        }
        Ok(())
    }

    fn close_rdp(&mut self) {
        self.processing_events.store(false, Ordering::Relaxed);
        self.rdp = None;
    }

    fn rdp_window_id(&self) -> Option<WindowId> {
        self.rdp.as_ref().map(|r| r.window.window.id())
    }
    fn launcher_window_id(&self) -> Option<WindowId> {
        self.launcher
            .as_ref()
            .and_then(|l| l.window.as_ref().map(|w| w.id()))
    }
    fn popup_window_id(&self) -> Option<WindowId> {
        self.popup.as_ref().map(|p| p.window.id())
    }
    fn about_window_id(&self) -> Option<WindowId> {
        self.about.as_ref().map(|a| a.window().id())
    }

    fn handle_rdp_input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CloseRequested => return false,
            WindowEvent::Resized(_) => {
                if let Some(s) = &mut self.rdp {
                    s.request_screen_resize();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.last_pointer = Some(*position);
                if let Some(s) = &mut self.rdp {
                    s.cursor_x = position.x as f32;
                    s.cursor_y = position.y as f32;
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
                    let _ = s.command_tx.send(rdp::commands::RdpCommand::Input(
                        rdp::commands::InputEvent::Mouse {
                            flags: rdp::sys::PTR_FLAGS_MOVE as u16,
                            x,
                            y,
                        },
                    ));
                    unsafe {
                        rdp::sys::SetEvent(s.command_event.as_handle());
                    }
                    // Pinbar
                    let is_fs = s.full_screen.load(Ordering::Relaxed);
                    // Show: in fullscreen AND cursor in center-top (< 5px Y, 40-60% X)
                    let show_trigger = position.y < 5.0
                        && position.x > s.window.window.inner_size().width as f64 * 0.4
                        && position.x < s.window.window.inner_size().width as f64 * 0.6;
                    if show_trigger {
                        s.pinbar_visible = is_fs;
                    }
                    // Hide: cursor leaves the pinbar area (Y > 32 px)
                    if position.y > 32.0 {
                        s.pinbar_visible = false;
                    }
                }
            }
            WindowEvent::MouseInput {
                state: btn, button, ..
            } => {
                // Pinbar button handling
                if let Some(pos) = self.last_pointer
                    && let Some(s) = &self.rdp
                    && s.pinbar_visible
                    && btn.is_pressed()
                    && *button == winit::event::MouseButton::Left
                {
                    let px = pos.x as f32;
                    if s.pinbar_btn_fs_x.contains(&px) {
                        self.toggle_fullscreen();
                        return true;
                    }
                    if s.pinbar_btn_close_x.contains(&px) {
                        return false; // Close RDP
                    }
                }

                if let Some(pos) = self.last_pointer
                    && let Some(s) = &self.rdp
                {
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
                        winit::event::MouseButton::Left => rdp::sys::PTR_FLAGS_BUTTON1,
                        winit::event::MouseButton::Right => rdp::sys::PTR_FLAGS_BUTTON2,
                        winit::event::MouseButton::Middle => rdp::sys::PTR_FLAGS_BUTTON3,
                        _ => 0,
                    } as u16;
                    if flags != 0 {
                        let f = flags
                            | if btn.is_pressed() {
                                rdp::sys::PTR_FLAGS_DOWN as u16
                            } else {
                                0
                            };
                        let _ = s.command_tx.send(rdp::commands::RdpCommand::Input(
                            rdp::commands::InputEvent::Mouse { flags: f, x, y },
                        ));
                        unsafe {
                            rdp::sys::SetEvent(s.command_event.as_handle());
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(ref s) = self.rdp {
                    let dy = match delta {
                        winit::event::MouseScrollDelta::LineDelta(_, y) => *y as i32,
                        winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as i32,
                    };
                    let mut wheel_delta = (dy as f32 * 120.0) as i32;
                    let flags = (rdp::sys::PTR_FLAGS_WHEEL as u16)
                        | if wheel_delta < 0 {
                            wheel_delta = -wheel_delta;
                            rdp::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16
                        } else {
                            0
                        };
                    while wheel_delta > 0 {
                        let step: u16 = if wheel_delta > 0xFF {
                            0xFF
                        } else {
                            (wheel_delta & 0xFF) as u16
                        };
                        wheel_delta -= step as i32;
                        let cflags = if flags & (rdp::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16) != 0 {
                            flags | (0x100 - step)
                        } else {
                            flags | step
                        };
                        let _ = s.command_tx.send(rdp::commands::RdpCommand::Input(
                            rdp::commands::InputEvent::Mouse {
                                flags: cflags,
                                x: 0,
                                y: 0,
                            },
                        ));
                        unsafe {
                            rdp::sys::SetEvent(s.command_event.as_handle());
                        }
                    }
                }
            }
            _ => {}
        }
        true
    }

    fn toggle_fullscreen(&mut self) {
        if let Some(ref mut s) = self.rdp {
            let is_fs = s.full_screen.load(Ordering::Relaxed);
            if !is_fs {
                s.window
                    .window
                    .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                s.full_screen.store(true, Ordering::Relaxed);
            } else {
                s.window.window.set_fullscreen(None);
                s.full_screen.store(false, Ordering::Relaxed);
            }
        }
    }
}

impl ApplicationHandler for AppHandler {
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
    }

    fn window_event(&mut self, el: &ActiveEventLoop, wid: WindowId, event: WindowEvent) {
        // Keyboard interception for RDP
        if self.rdp.is_some()
            && let WindowEvent::KeyboardInput { event: key_ev, .. } = &event
            && let PhysicalKey::Code(code) = key_ev.physical_key
        {
            match code {
                winit::keyboard::KeyCode::AltLeft | winit::keyboard::KeyCode::AltRight => {
                    self.alt_held = key_ev.state.is_pressed();
                }
                _ => {}
            }
            if self.processing_events.load(Ordering::Relaxed) {
                if self.alt_held && key_ev.state.is_pressed() && !key_ev.repeat {
                    match code {
                        winit::keyboard::KeyCode::Enter => {
                            log::debug!("Alt+Enter → fullscreen");
                            self.toggle_fullscreen();
                            return;
                        }
                        winit::keyboard::KeyCode::KeyF => {
                            if let Some(ref s) = self.rdp {
                                s.fps.toggle();
                            }
                            return;
                        }
                        winit::keyboard::KeyCode::F4 => {
                            log::debug!("Alt+F4 → exit");
                            self.stop.trigger();
                            el.exit();
                            return;
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
                return;
            }
        }

        let should_redraw = matches!(&event, WindowEvent::RedrawRequested);

        // Dispatch by window
        if Some(wid) == self.launcher_window_id() {
            match event {
                WindowEvent::CloseRequested => {
                    self.stop.trigger();
                    el.exit();
                }
                WindowEvent::MouseInput { state, button, .. }
                    if state.is_pressed() && button == winit::event::MouseButton::Left =>
                {
                    if let Some(ref mut l) = self.launcher
                        && let Some(pos) = l.last_mouse_pos
                    {
                        l.inner.handle_click(pos.0, pos.1);
                    }
                    if let Some(ref l) = self.launcher
                        && let Some(w) = &l.window
                    {
                        w.request_redraw();
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    if let Some(ref mut l) = self.launcher {
                        let sf = *monitor::SCALE_FACTOR as f32;
                        l.last_mouse_pos = Some((position.x as f32 / sf, position.y as f32 / sf));
                    }
                }
                _ => {}
            }
            if should_redraw && let Some(ref mut l) = self.launcher {
                paint_launcher(l);
            }
        } else if Some(wid) == self.rdp_window_id() {
            if self.rdp.as_ref().map(|s| s.is_rail).unwrap_or(false) {
                if let WindowEvent::CloseRequested = event {
                    self.stop.trigger();
                    el.exit();
                }
            } else {
                let ok = self.handle_rdp_input(&event);
                if !ok {
                    self.stop.trigger();
                    el.exit();
                }
                if should_redraw && let Some(ref mut s) = self.rdp {
                    let _ = s.update_screen();
                }
            }
        } else if Some(wid) == self.popup_window_id() {
            match &event {
                WindowEvent::CloseRequested => {
                    self.popup = None;
                }
                WindowEvent::MouseInput { state, button, .. }
                    if state.is_pressed() && *button == winit::event::MouseButton::Left =>
                {
                    // Use last_pointer if it was set on this window
                    if let Some(pos) = self.last_pointer
                        && let Some(ref mut p) = self.popup
                        && p.handle_click(pos.x as f32, pos.y as f32)
                    {
                        self.popup = None;
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.last_pointer = Some(*position);
                }
                _ => {}
            }
            if should_redraw && let Some(ref mut p) = self.popup {
                p.paint();
            }
        } else if Some(wid) == self.about_window_id() {
            match event {
                WindowEvent::CloseRequested => {
                    self.about = None;
                }
                WindowEvent::MouseInput { state, button, .. }
                    if state.is_pressed() && button == winit::event::MouseButton::Left =>
                {
                    self.about = None;
                }
                _ => {}
            }
            if should_redraw && let Some(ref mut a) = self.about {
                a.paint();
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // Process test action requests from launcher
        if let Some(ref mut launcher) = self.launcher
            && let Some(action) = launcher.inner.take_request()
        {
            match action {
                TestAction::ShowProgress => {
                    launcher.inner = LauncherInner::Progress {
                        pct: 0,
                        message: String::new(),
                    };
                }
                TestAction::GoInvisible => {
                    launcher.inner = LauncherInner::Invisible;
                }
                TestAction::ShowWarning => {
                    launcher.inner = LauncherInner::Warning("This is a warning message.".into());
                }
                TestAction::ShowError => {
                    launcher.inner = LauncherInner::Error("This is an error message.".into());
                }
                TestAction::ShowYesNo => {
                    let (resp_tx, _) = tokio::sync::oneshot::channel::<bool>();
                    launcher.inner = LauncherInner::YesNo {
                        message: "Do you want to continue?".into(),
                        response: Arc::new(std::sync::RwLock::new(Some(resp_tx))),
                    };
                }
                TestAction::ConnectRdp
                | TestAction::ConnectRdpPreconnection
                | TestAction::ConnectRail => {
                    let is_rail = matches!(action, TestAction::ConnectRail);
                    let settings = rdp::settings::RdpSettings {
                        server: "172.27.247.161".to_string(),
                        user: "user".to_string(),
                        password: "temporal".to_string(),
                        screen_size: rdp::geom::ScreenSize::Full,
                        rail_app: if is_rail {
                            Some("c:\\windows\\notepad.exe".to_string())
                        } else {
                            None
                        },
                        ..Default::default()
                    };
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

        // Process external GUI messages
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
                        self.popup = Some(p);
                    }
                }
                GuiMessage::ShowWarning(msg) => {
                    if let Ok(p) = PopupState::new(el, PopupKind::Warning(msg)) {
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

        // Process RDP updates
        if let Some(ref mut state) = self.rdp {
            while let Ok(message) = state.update_rx.try_recv() {
                match handle_rdp_message(state, message) {
                    session::RdpActionResult::Continue if !state.is_rail => {
                        state.window.window.request_redraw();
                    }
                    session::RdpActionResult::Disconnect => {
                        self.stop.trigger();
                        self.return_code = ReturnCode::Exit;
                        self.close_rdp();
                        el.exit();
                        return;
                    }
                    session::RdpActionResult::Error(_) => {
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
                    let _ = state.command_tx.send(rdp::commands::RdpCommand::Input(
                        rdp::commands::InputEvent::Keyboard {
                            scancode: sc as u16,
                            pressed: raw_key.pressed,
                        },
                    ));
                    unsafe {
                        rdp::sys::SetEvent(state.command_event.as_handle());
                    }
                }
            }
            state.fps.record();
        }

        // Redraw popup if visible
        if let Some(ref p) = self.popup {
            p.window.request_redraw();
        }
        if let Some(ref a) = self.about {
            a.window().request_redraw();
        }
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.stop.trigger();
    }
}
