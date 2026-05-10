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

use launcher::{LauncherInner, LauncherState, paint_launcher};
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
    RdpSession(RdpState),
}

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
        let context = softbuffer::Context::new(window.clone())
            .map_err(|e| anyhow::anyhow!("Softbuffer context error: {e}"))?;
        let mut surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|e| anyhow::anyhow!("Softbuffer surface error: {e}"))?;
        surface
            .resize(NonZeroU32::new(400).unwrap(), NonZeroU32::new(300).unwrap())
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
        };
        self.phase = Phase::Launcher(launcher);
        Ok(())
    }

    fn enter_rdp(
        &mut self,
        event_loop: &ActiveEventLoop,
        settings: rdp::settings::RdpSettings,
    ) -> Result<()> {
        let is_rail = settings.rail_app.is_some();
        let use_rgba = cfg!(target_os = "macos");

        if is_rail {
            let sf = 1.0;
            let (width, height) = monitor::size(0).unwrap_or((1920, 1080));

            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("UDS RemoteApp")
                        .with_inner_size(winit::dpi::LogicalSize::new(300.0, 100.0))
                        .with_window_icon(Some(logo::load_icon())),
                )?,
            );

            let ctx = softbuffer::Context::new(window.clone())
                .map_err(|e| anyhow::anyhow!("Softbuffer context error: {e}"))?;
            let mut surface = softbuffer::Surface::new(&ctx, window.clone())
                .map_err(|e| anyhow::anyhow!("Softbuffer surface error: {e}"))?;
            surface
                .resize(NonZeroU32::new(300).unwrap(), NonZeroU32::new(100).unwrap())
                .map_err(|e| anyhow::anyhow!("Softbuffer resize error: {e}"))?;

            let rdp_window = RdpWindow {
                window,
                surface,
                context: ctx,
                scratch: Vec::new(),
            };

            let rdp_state = RdpState::new(
                rdp_window,
                settings,
                true,
                sf,
                (width, height),
                self.keys_rx.clone(),
                use_rgba,
            )?;
            self.phase = Phase::RdpSession(rdp_state);
        } else {
            let sf = 1.0;

            let (screen_w, screen_h) = match settings.screen_size {
                rdp::geom::ScreenSize::Full => {
                    let (mw, mh) = monitor::size(0).unwrap_or((1920, 1080));
                    (mw, mh)
                }
                rdp::geom::ScreenSize::Fixed(w, h) => (w, h),
            };

            let lw = screen_w as f64 / sf;
            let lh = screen_h as f64 / sf;

            let window = Arc::new(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_title("UDS Remote Desktop")
                        .with_inner_size(winit::dpi::LogicalSize::new(lw, lh))
                        .with_window_icon(Some(logo::load_icon())),
                )?,
            );

            let ctx = softbuffer::Context::new(window.clone())
                .map_err(|e| anyhow::anyhow!("Softbuffer context error: {e}"))?;
            let mut surface = softbuffer::Surface::new(&ctx, window.clone())
                .map_err(|e| anyhow::anyhow!("Softbuffer surface error: {e}"))?;
            surface
                .resize(
                    NonZeroU32::new(screen_w).unwrap(),
                    NonZeroU32::new(screen_h).unwrap(),
                )
                .map_err(|e| anyhow::anyhow!("Softbuffer resize error: {e}"))?;

            let rdp_window = RdpWindow {
                window,
                surface,
                context: ctx,
                scratch: Vec::new(),
            };

            let rdp_state = RdpState::new(
                rdp_window,
                settings,
                false,
                sf,
                (screen_w, screen_h),
                self.keys_rx.clone(),
                use_rgba,
            )?;
            self.phase = Phase::RdpSession(rdp_state);
        }

        self.processing_events.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// Handle RDP normal session input events
    fn handle_normal_input(state: &RdpState, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::CloseRequested => return false,
            WindowEvent::CursorMoved { position, .. } => {
                let scale = state.scale_factor as f32;
                let gdi_w = unsafe { (*state.gdi).width as i32 };
                let gdi_h = unsafe { (*state.gdi).height as i32 };
                let x = (position.x as f32 * scale) as i32;
                let y = (position.y as f32 * scale) as i32;
                if x >= 0 && y >= 0 && x < gdi_w && y < gdi_h {
                    let _ = state.command_tx.send(rdp::commands::RdpCommand::Input(
                        rdp::commands::InputEvent::Mouse {
                            flags: rdp::sys::PTR_FLAGS_MOVE as u16,
                            x: x as u16,
                            y: y as u16,
                        },
                    ));
                    unsafe {
                        rdp::sys::SetEvent(state.command_event.as_handle());
                    }
                }
            }
            WindowEvent::MouseInput {
                state: btn, button, ..
            } => {
                let flags = match button {
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
                    let _ = state.command_tx.send(rdp::commands::RdpCommand::Input(
                        rdp::commands::InputEvent::Mouse {
                            flags: f,
                            x: 0,
                            y: 0,
                        },
                    ));
                    unsafe {
                        rdp::sys::SetEvent(state.command_event.as_handle());
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y as i32,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as i32,
                };
                let wheel = (dy as f32 * 120.0) as i32;
                let mut rem = wheel.abs();
                let neg = wheel < 0;
                while rem > 0 {
                    let step = rem.min(0xFF) as u16;
                    rem -= step as i32;
                    let f = rdp::sys::PTR_FLAGS_WHEEL as u16
                        | if neg {
                            rdp::sys::PTR_FLAGS_WHEEL_NEGATIVE as u16
                        } else {
                            0
                        }
                        | step;
                    let _ = state.command_tx.send(rdp::commands::RdpCommand::Input(
                        rdp::commands::InputEvent::Mouse {
                            flags: f,
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
        true
    }
}

impl ApplicationHandler for AppHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        monitor::populate(event_loop);

        if self.first_resume {
            self.first_resume = false;
            let initial = self.initial_state.take().unwrap_or_default();
            let inner = match initial {
                AppState::Test => LauncherInner::Test,
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
        // ── Step 1: Keyboard interception (global, for RDP) ──
        if let Phase::RdpSession(_) = self.phase {
            if let WindowEvent::KeyboardInput { event: key_ev, .. } = &event {
                if let PhysicalKey::Code(code) = key_ev.physical_key {
                    if self.processing_events.load(Ordering::Relaxed) {
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
            }
        }

        // ── Step 2: Dispatch by phase ──
        match &mut self.phase {
            Phase::Launcher(launcher) => {
                let win_id = launcher.window.as_ref().map(|w| w.id());
                if win_id == Some(window_id) {
                    match event {
                        WindowEvent::CloseRequested => {
                            self.stop.trigger();
                            event_loop.exit();
                        }
                        WindowEvent::RedrawRequested => {
                            paint_launcher(launcher);
                        }
                        WindowEvent::MouseInput { state, button, .. } => {
                            if state.is_pressed() && button == winit::event::MouseButton::Left {
                                if let Some(pos) = launcher.last_mouse_pos {
                                    launcher.inner.handle_click(pos.0, pos.1);
                                }
                                if let Some(w) = &launcher.window {
                                    let _ = w.request_redraw();
                                }
                            }
                        }
                        WindowEvent::CursorMoved { position, .. } => {
                            launcher.last_mouse_pos = Some((position.x as f32, position.y as f32));
                        }
                        _ => {}
                    }
                }
            }
            Phase::RdpSession(state) => {
                let is_rail = state.is_rail;
                let main_win = state.window.window.id();

                if is_rail {
                    // RAIL mode: events go to individual RAIL windows
                    if window_id == main_win {
                        // Main RAIL status window
                        if let WindowEvent::CloseRequested = event {
                            self.stop.trigger();
                            event_loop.exit();
                        }
                    }
                    // RAIL window events handled by the window itself via redraw
                } else {
                    // Normal RDP session: single window
                    if window_id == main_win {
                        let should_continue = Self::handle_normal_input(state, &event);
                        if !should_continue {
                            self.stop.trigger();
                            event_loop.exit();
                        }
                        if let WindowEvent::RedrawRequested = &event {
                            let _ = state.update_screen();
                        }
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // ── Process external GUI messages ──
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
                            let _ = w.request_redraw();
                        }
                    }
                }
                GuiMessage::ShowWarning(msg) => {
                    if let Phase::Launcher(ref mut launcher) = self.phase {
                        launcher.inner = LauncherInner::Warning(msg);
                        if let Some(w) = &launcher.window {
                            w.set_visible(true);
                            let _ = w.request_redraw();
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
                            let _ = w.request_redraw();
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
                            let _ = w.request_redraw();
                        }
                    }
                }
                GuiMessage::Progress(pct, msg) => {
                    if let Phase::Launcher(ref mut launcher) = self.phase {
                        launcher.inner = LauncherInner::Progress { pct, message: msg };
                        if let Some(w) = &launcher.window {
                            let _ = w.request_redraw();
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
                        let _ = state.window.window.request_redraw();
                    }
                }
            }
        }

        // ── Process RDP updates ──
        if let Phase::RdpSession(ref mut state) = self.phase {
            while let Ok(message) = state.update_rx.try_recv() {
                match handle_rdp_message(state, message) {
                    session::RdpActionResult::Continue => {
                        if !state.is_rail {
                            let _ = state.window.window.request_redraw();
                        }
                    }
                    session::RdpActionResult::Disconnect => {
                        self.stop.trigger();
                        self.return_code = ReturnCode::Exit;
                        self.processing_events.store(false, Ordering::Relaxed);
                        event_loop.exit();
                        return;
                    }
                    session::RdpActionResult::Error(_) => {
                        self.stop.trigger();
                        self.return_code = ReturnCode::Exit;
                        self.processing_events.store(false, Ordering::Relaxed);
                        event_loop.exit();
                        return;
                    }
                    _ => {}
                }
            }

            // Process raw keyboard events → RDP
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
