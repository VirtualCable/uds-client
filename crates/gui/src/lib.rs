// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use anyhow::Result;
use flume::{Receiver, Sender, bounded};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::WindowId;

use shared::system::trigger::Trigger;

pub mod about;
pub mod keymap;
pub mod logo;
mod monitor;
mod popup;
pub mod types;

mod draw;
pub mod ipc;
mod launcher;
mod progress_window;
mod rdp;
mod wgpu_render;

mod input;
mod launcher_ui;
mod session;

use launcher::{LauncherInner, TestingLauncherState};
use popup::PopupState;
use rdp::RdpState;
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
    Progress,
}

pub struct AppHandler {
    launcher: Option<TestingLauncherState>,
    progress: Option<crate::progress_window::ProgressState>,
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
    rail_ipc: Option<crate::ipc::IpcListener>,
    pacing_started: bool,
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
        progress: None,
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
        rail_ipc: None,
        pacing_started: false,
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
}

// ── ApplicationHandler ──────

impl ApplicationHandler<UserEvent> for AppHandler {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        monitor::populate(el);
        if self.first_resume {
            self.first_resume = false;
            let inner = match self.initial_state.take().unwrap_or_default() {
                #[cfg(feature = "test-ui")]
                AppState::Test => LauncherInner::new_test(),
                #[cfg(not(feature = "test-ui"))]
                AppState::Progress => LauncherInner::default(),
            };
            let _ = self.open_launcher(el, inner);
        }
        // Start frame pacing once
        if !self.pacing_started {
            self.pacing_started = true;
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
                    Some(WindowKind::Progress) => {
                        self.handle_progress_event(el, WindowEvent::RedrawRequested)
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
                    Some(WindowKind::Popup) => {
                        self.handle_popup_event(WindowEvent::RedrawRequested)
                    }
                    _ => {}
                }
            }
            _ => {
                // Dispatch by window kind
                match self.windows.get(&wid) {
                    Some(WindowKind::Launcher) => self.handle_launcher_event(el, event),
                    Some(WindowKind::Progress) => self.handle_progress_event(el, event),
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
                if let Some(ref mut r) = self.rdp {
                    r.window.window.request_redraw();
                }
                if let Some(ref mut p) = self.popup {
                    p.window.request_redraw();
                }
                if let Some(ref mut l) = self.launcher
                    && let Some(ref w) = l.window
                {
                    w.request_redraw();
                }
                if let Some(ref mut p) = self.progress {
                    p.animation_time += 0.3; // Slightly faster waves
                    // Simulación de progreso solo en modo testing
                    if self.launcher.is_some() && p.pct < 100 {
                        p.pct += 1;
                        if p.pct >= 100 {
                            p.phase = crate::progress_window::ProgressPhase::Connected;
                        }
                    }
                    p.window.request_redraw();
                }
                if let Some(ref mut a) = self.about {
                    a.animation_time += 0.3;
                    a.window().request_redraw();
                }
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.stop.is_triggered() {
            el.exit();
        }
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.stop.trigger();
    }
}
