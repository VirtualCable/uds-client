// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

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
pub mod types;
pub mod window;

mod launcher;
mod session;
mod wgpu_render;

use launcher::{LauncherInner, LauncherState, TestAction, paint_launcher};
use session::{RdpState, RdpWindow, handle_rdp_message};
use types::{AppState, GuiMessage, ReturnCode};

#[derive(Debug)]
pub struct RawKey {
    pub keycode: winit::keyboard::KeyCode,
    pub pressed: bool,
    pub repeat: bool,
}

enum Phase {
    Launcher(LauncherState),
    RdpSession(Box<RdpState>),
}

#[allow(dead_code)]
pub struct AppHandler {
    phase: Phase,
    keys_tx: Sender<RawKey>,
    keys_rx: Receiver<RawKey>,
    gui_messages_rx: Receiver<GuiMessage>,
    processing_events: Arc<AtomicBool>,
    stop: Trigger,
    fps_limit: Option<u32>,
    catalog: gettext::Catalog,
    last_frame: Instant,
    return_code: ReturnCode,
    initial_state: Option<AppState>,
    first_resume: bool,
    alt_held: bool,
    last_pointer: Option<winit::dpi::PhysicalPosition<f64>>,
}

pub fn run_gui(
    catalog: gettext::Catalog,
    initial_state: Option<AppState>,
    messages_rx: Receiver<GuiMessage>,
    stop: Trigger,
    fps_limit: Option<u32>,
) -> Result<ReturnCode> {
    let (keys_tx, keys_rx) = bounded::<RawKey>(1024);
    let processing_events = Arc::new(AtomicBool::new(false));

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = AppHandler {
        phase: Phase::Launcher(LauncherState::new()),
        keys_tx,
        keys_rx,
        gui_messages_rx: messages_rx,
        processing_events,
        stop,
        fps_limit,
        catalog,
        last_frame: Instant::now(),
        return_code: ReturnCode::Exit,
        initial_state,
        first_resume: true,
        alt_held: false,
        last_pointer: None,
    };

    event_loop.run_app(&mut app)?;

    Ok(app.return_code)
}

impl AppHandler {
    fn create_launcher_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        inner: LauncherInner,
    ) -> Result<()> {
        let window_attrs = Window::default_attributes()
            .with_title("UDS Launcher")
            .with_inner_size(winit::dpi::LogicalSize::new(400.0, 300.0))
            .with_window_icon(Some(logo::load_icon()))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(window_attrs)?);
        let physical = window.inner_size();
        let win_scale = window.scale_factor() as f32;
        let context = softbuffer::Context::new(window.clone())
            .map_err(|e| anyhow::anyhow!("Softbuffer context error: {e}"))?;
        let (ph_w, ph_h) = (
            NonZeroU32::new(physical.width).unwrap_or(NonZeroU32::new(1).unwrap()),
            NonZeroU32::new(physical.height).unwrap_or(NonZeroU32::new(1).unwrap()),
        );
        let mut surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|e| anyhow::anyhow!("Softbuffer surface error: {e}"))?;
        surface
            .resize(ph_w, ph_h)
            .map_err(|e| anyhow::anyhow!("Softbuffer resize error: {e}"))?;

        let l = logo::load_logo();
        let launcher = LauncherState {
            window: Some(window),
            surface: Some(surface),
            context: Some(context),
            logo_rgba: l.rgba,
            logo_width: l.width,
            logo_height: l.height,
            inner,
            last_mouse_pos: None,
            phys_w: physical.width,
            phys_h: physical.height,
            scale_factor: win_scale,
        };
        self.phase = Phase::Launcher(launcher);
        Ok(())
    }

    fn enter_rdp(
        &mut self,
        event_loop: &ActiveEventLoop,
        mut settings: rdp::settings::RdpSettings,
    ) -> Result<()> {
        let is_rail = settings.rail_app.is_some();
        let use_rgba = cfg!(target_os = "macos");

        let (desktop_w, desktop_h) = monitor::size(0).unwrap_or((1920, 1080));
        let monitor_scale = monitor::scale(0);

        // Use logical resolution for RDP: physical / scale → saves bandwidth on HiDPI
        let (rdp_w, rdp_h) = match settings.screen_size {
            rdp::geom::ScreenSize::Full => (
                (desktop_w as f64 / monitor_scale) as u32,
                (desktop_h as f64 / monitor_scale) as u32,
            ),
            rdp::geom::ScreenSize::Fixed(w, h) => (w, h),
        };

        settings.scale_factor = monitor_scale;
        let is_fullscreen = settings.screen_size.is_fullscreen() && !is_rail;

        log::info!(
            "enter_rdp: rail={is_rail} fullscreen={is_fullscreen} logical={rdp_w}x{rdp_h} scale={monitor_scale} desktop={desktop_w}x{desktop_h}"
        );

        // Window at physical size, RDP framebuffer at logical size
        let window_logical_w = desktop_w as f64 / monitor_scale;
        let window_logical_h = desktop_h as f64 / monitor_scale;

        if is_rail {
            settings.screen_size = rdp::geom::ScreenSize::Fixed(rdp_w, rdp_h);

            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("UDS RemoteApp")
                        .with_inner_size(winit::dpi::LogicalSize::new(300.0, 100.0))
                        .with_window_icon(Some(logo::load_icon())),
                )?,
            );

            let renderer = crate::wgpu_render::WgpuRenderer::new(window.clone(), 300, 100)?;

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
            self.phase = Phase::RdpSession(Box::new(rdp_state));
        } else {
            settings.screen_size = rdp::geom::ScreenSize::Fixed(rdp_w, rdp_h);

            let window = Arc::new(
                event_loop.create_window(
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

            self.phase = Phase::RdpSession(Box::new(rdp_state));
        }

        while self.keys_rx.try_recv().is_ok() {}
        self.processing_events.store(true, Ordering::Relaxed);
        if let Phase::RdpSession(ref state) = self.phase {
            state.window.window.set_cursor_visible(false);
        }
        Ok(())
    }

    fn toggle_fullscreen(state: &mut RdpState) {
        let is_fs = state.full_screen.load(Ordering::Relaxed);
        if !is_fs {
            state
                .window
                .window
                .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            state.full_screen.store(true, Ordering::Relaxed);
        } else {
            state.window.window.set_fullscreen(None);
            state.full_screen.store(false, Ordering::Relaxed);
        }
        // resize will be picked up by Resized event → request_screen_resize
    }
}

impl ApplicationHandler for AppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        monitor::populate(event_loop);
        if self.first_resume {
            self.first_resume = false;
            let initial = self.initial_state.take().unwrap_or_default();
            let inner = match initial {
                AppState::Test => LauncherInner::new_test(),
                AppState::Invisible => LauncherInner::Invisible,
            };
            let _ = self.create_launcher_window(event_loop, inner);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // ── Keyboard interception + hotkeys ──
        if let Phase::RdpSession(_) = self.phase
            && let WindowEvent::KeyboardInput { event: key_ev, .. } = &event
            && let PhysicalKey::Code(code) = key_ev.physical_key
        {
            // Track Alt key state for hotkeys
            match code {
                winit::keyboard::KeyCode::AltLeft | winit::keyboard::KeyCode::AltRight => {
                    self.alt_held = key_ev.state.is_pressed();
                }
                _ => {}
            }

            if self.processing_events.load(Ordering::Relaxed) {
                // Hotkeys: Alt+Enter (fullscreen), Alt+F (FPS toggle), Alt+F4 (exit)
                if let Phase::RdpSession(ref mut state) = self.phase
                    && !state.is_rail
                    && self.alt_held
                    && key_ev.state.is_pressed()
                    && !key_ev.repeat
                {
                    match code {
                        winit::keyboard::KeyCode::Enter => {
                            log::debug!("Hotkey: Alt+Enter → toggle fullscreen");
                            Self::toggle_fullscreen(state);
                            return;
                        }
                        winit::keyboard::KeyCode::KeyF => {
                            log::debug!("Hotkey: Alt+F → toggle FPS");
                            state.fps.toggle();
                            return;
                        }
                        winit::keyboard::KeyCode::F4 => {
                            log::debug!("Hotkey: Alt+F4 → exit RDP");
                            self.stop.trigger();
                            event_loop.exit();
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
                if let Err(e) = self.keys_tx.send(raw) {
                    log::warn!("Failed to send keyboard event: {}", e);
                }
                return;
            }
        }

        let should_redraw = matches!(&event, WindowEvent::RedrawRequested);

        match &mut self.phase {
            Phase::Launcher(launcher) => {
                let win_id = launcher.window.as_ref().map(|w| w.id());
                if win_id != Some(window_id) {
                    return;
                }
                match event {
                    WindowEvent::CloseRequested => {
                        self.stop.trigger();
                        event_loop.exit();
                    }
                    WindowEvent::MouseInput { state, button, .. }
                        if state.is_pressed() && button == winit::event::MouseButton::Left =>
                    {
                        if let Some(pos) = launcher.last_mouse_pos {
                            launcher.inner.handle_click(pos.0, pos.1);
                        }
                        if let Some(w) = &launcher.window {
                            w.request_redraw();
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        let sf = launcher.scale_factor;
                        launcher.last_mouse_pos =
                            Some((position.x as f32 / sf, position.y as f32 / sf));
                    }
                    _ => {}
                }
                if should_redraw {
                    paint_launcher(launcher);
                }
            }
            Phase::RdpSession(state) => {
                let is_rail = state.is_rail;
                let main_win = state.window.window.id();

                if is_rail {
                    if window_id == main_win
                        && let WindowEvent::CloseRequested = event
                    {
                        self.stop.trigger();
                        event_loop.exit();
                    }
                } else if window_id == main_win {
                    match &event {
                        WindowEvent::CloseRequested => {
                            self.stop.trigger();
                            event_loop.exit();
                        }
                        WindowEvent::Resized(_) => {
                            state.request_screen_resize();
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            state.cursor_x = position.x as f32;
                            state.cursor_y = position.y as f32;
                            self.last_pointer = Some(*position);
                            state.window.window.request_redraw();
                            let gdi_w = unsafe { (*state.gdi).width as u32 };
                            let gdi_h = unsafe { (*state.gdi).height as u32 };
                            let phys_w = state.window.window.inner_size().width;
                            let phys_h = state.window.window.inner_size().height;
                            // Map physical window coords → GDI logical coords (round for HiDPI)
                            let x = ((position.x * gdi_w as f64) / phys_w as f64)
                                .round()
                                .clamp(0.0, (gdi_w - 1) as f64)
                                as u16;
                            let y = ((position.y * gdi_h as f64) / phys_h as f64)
                                .round()
                                .clamp(0.0, (gdi_h - 1) as f64)
                                as u16;
                            let _ = state.command_tx.send(rdp::commands::RdpCommand::Input(
                                rdp::commands::InputEvent::Mouse {
                                    flags: rdp::sys::PTR_FLAGS_MOVE as u16,
                                    x,
                                    y,
                                },
                            ));
                            unsafe {
                                rdp::sys::SetEvent(state.command_event.as_handle());
                            }
                        }
                        WindowEvent::MouseInput {
                            state: btn, button, ..
                        } => {
                            if let Some(pos) = self.last_pointer {
                                let gdi_w = unsafe { (*state.gdi).width as u32 };
                                let gdi_h = unsafe { (*state.gdi).height as u32 };
                                let phys_w = state.window.window.inner_size().width;
                                let phys_h = state.window.window.inner_size().height;
                                let x = ((pos.x * gdi_w as f64) / phys_w as f64)
                                    .round()
                                    .clamp(0.0, (gdi_w - 1) as f64)
                                    as u16;
                                let y = ((pos.y * gdi_h as f64) / phys_h as f64)
                                    .round()
                                    .clamp(0.0, (gdi_h - 1) as f64)
                                    as u16;
                                let flags = match button {
                                    winit::event::MouseButton::Left => rdp::sys::PTR_FLAGS_BUTTON1,
                                    winit::event::MouseButton::Right => rdp::sys::PTR_FLAGS_BUTTON2,
                                    winit::event::MouseButton::Middle => {
                                        rdp::sys::PTR_FLAGS_BUTTON3
                                    }
                                    _ => 0,
                                } as u16;
                                if flags != 0 {
                                    let f = flags
                                        | if btn.is_pressed() {
                                            rdp::sys::PTR_FLAGS_DOWN as u16
                                        } else {
                                            0
                                        };
                                    let _ =
                                        state.command_tx.send(rdp::commands::RdpCommand::Input(
                                            rdp::commands::InputEvent::Mouse { flags: f, x, y },
                                        ));
                                    unsafe {
                                        rdp::sys::SetEvent(state.command_event.as_handle());
                                    }
                                }
                            }
                        }
                        WindowEvent::MouseWheel { delta, .. } => {
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
                                let cflags =
                                    if flags & (rdp::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16) != 0 {
                                        flags | (0x100 - step)
                                    } else {
                                        flags | step
                                    };
                                let _ = state.command_tx.send(rdp::commands::RdpCommand::Input(
                                    rdp::commands::InputEvent::Mouse {
                                        flags: cflags,
                                        x: 0,
                                        y: 0,
                                    },
                                ));
                                unsafe {
                                    rdp::sys::SetEvent(state.command_event.as_handle());
                                }
                            }
                        }
                        _ => {}
                    }
                    if should_redraw {
                        let _ = state.update_screen();
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // ── Test actions from launcher ──
        if let Phase::Launcher(ref mut launcher) = self.phase
            && let Some(action) = launcher.inner.take_request()
        {
            match action {
                TestAction::ShowProgress => {
                    launcher.inner = LauncherInner::Progress {
                        pct: 0,
                        message: String::new(),
                    };
                    if let Some(w) = &launcher.window {
                        w.set_visible(true);
                        w.request_redraw();
                    }
                }
                TestAction::GoInvisible => {
                    launcher.inner = LauncherInner::Invisible;
                    if let Some(w) = &launcher.window {
                        w.set_visible(false);
                    }
                }
                TestAction::ShowWarning => {
                    launcher.inner = LauncherInner::Warning("This is a warning message.".into());
                    if let Some(w) = &launcher.window {
                        w.request_redraw();
                    }
                }
                TestAction::ShowError => {
                    launcher.inner = LauncherInner::Error("This is an error message.".into());
                    if let Some(w) = &launcher.window {
                        w.request_redraw();
                    }
                }
                TestAction::ShowYesNo => {
                    let (resp_tx, _) = tokio::sync::oneshot::channel::<bool>();
                    launcher.inner = LauncherInner::YesNo {
                        message: "Do you want to continue?".into(),
                        response: Arc::new(std::sync::RwLock::new(Some(resp_tx))),
                    };
                    if let Some(w) = &launcher.window {
                        w.request_redraw();
                    }
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
                    self.phase = Phase::Launcher(LauncherState::new());
                    if let Err(e) = self.enter_rdp(event_loop, settings) {
                        log::error!("Failed to enter RDP: {}", e);
                        self.stop.trigger();
                        event_loop.exit();
                        return;
                    }
                    if let Phase::RdpSession(ref state) = self.phase {
                        state.window.window.request_redraw();
                    }
                }
            }
        }

        // ── External GUI messages ──
        while let Ok(msg) = self.gui_messages_rx.try_recv() {
            match msg {
                GuiMessage::Close => {
                    self.stop.trigger();
                    event_loop.exit();
                    return;
                }
                GuiMessage::Hide => {
                    if let Phase::Launcher(ref mut launcher) = self.phase {
                        launcher.inner = LauncherInner::Invisible;
                        if let Some(w) = &launcher.window {
                            w.set_visible(false);
                        }
                    }
                }
                GuiMessage::ShowError(err) => {
                    if let Phase::Launcher(ref mut launcher) = self.phase {
                        launcher.inner = LauncherInner::Error(err);
                        if let Some(w) = &launcher.window {
                            w.set_visible(true);
                            w.request_redraw();
                        }
                    }
                }
                GuiMessage::ShowWarning(msg) => {
                    if let Phase::Launcher(ref mut launcher) = self.phase {
                        launcher.inner = LauncherInner::Warning(msg);
                        if let Some(w) = &launcher.window {
                            w.set_visible(true);
                            w.request_redraw();
                        }
                    }
                }
                GuiMessage::ShowYesNo(msg, resp) => {
                    if let Phase::Launcher(ref mut launcher) = self.phase {
                        launcher.inner = LauncherInner::YesNo {
                            message: msg,
                            response: resp,
                        };
                        if let Some(w) = &launcher.window {
                            w.set_visible(true);
                            w.request_redraw();
                        }
                    }
                }
                GuiMessage::ShowProgress => {
                    if let Phase::Launcher(ref mut launcher) = self.phase {
                        launcher.inner = LauncherInner::Progress {
                            pct: 0,
                            message: String::new(),
                        };
                        if let Some(w) = &launcher.window {
                            w.set_visible(true);
                            w.request_redraw();
                        }
                    }
                }
                GuiMessage::Progress(pct, msg) => {
                    if let Phase::Launcher(ref mut launcher) = self.phase {
                        launcher.inner = LauncherInner::Progress { pct, message: msg };
                        if let Some(w) = &launcher.window {
                            w.request_redraw();
                        }
                    }
                }
                GuiMessage::ConnectRdp(settings) => {
                    self.phase = Phase::Launcher(LauncherState::new());
                    if let Err(e) = self.enter_rdp(event_loop, settings) {
                        log::error!("Failed to enter RDP: {}", e);
                        self.stop.trigger();
                        event_loop.exit();
                        return;
                    }
                    if let Phase::RdpSession(ref state) = self.phase {
                        state.window.window.request_redraw();
                    }
                }
            }
        }

        // ── RDP updates ──
        if let Phase::RdpSession(ref mut state) = self.phase {
            while let Ok(message) = state.update_rx.try_recv() {
                match handle_rdp_message(state, message) {
                    session::RdpActionResult::Continue if !state.is_rail => {
                        state.window.window.request_redraw();
                    }
                    session::RdpActionResult::Disconnect => {
                        self.stop.trigger();
                        self.return_code = ReturnCode::Exit;
                        self.processing_events.store(false, Ordering::Relaxed);
                        event_loop.exit();
                        return;
                    }
                    session::RdpActionResult::Error(_err) => {
                        self.stop.trigger();
                        self.return_code = ReturnCode::Exit;
                        self.processing_events.store(false, Ordering::Relaxed);
                        event_loop.exit();
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
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.stop.trigger();
    }
}
