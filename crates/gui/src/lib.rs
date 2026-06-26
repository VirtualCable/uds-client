// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

extern crate rdp;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use anyhow::Result;
use flume::{Receiver, Sender, bounded};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use shared::system::trigger::Trigger;

pub mod keymap;
pub mod logo;
mod monitor;
pub mod types;
pub mod windows;

mod draw;
pub mod ipc;
mod wgpu_render;

mod input;
mod session;

use types::{AppState, GuiMessage, ReturnCode};
use windows::about::AboutState;
use windows::popup::PopupState;
use windows::progress::{ProgressPhase, ProgressState};
use windows::rdp_window::{RdpMode, RdpState};
use windows::testing_launcher_window::TestingLauncherState;

#[derive(Debug)]
pub struct RawKey {
    pub keycode: winit::keyboard::KeyCode,
    pub pressed: bool,
    pub repeat: bool,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowKind {
    TestingLauncher,
    Rdp,
    RdpRail(u32),
    Popup,
    About,
    Progress,
}

pub struct AppHandler {
    catalog: gettext::Catalog,
    testing_launcher: Option<TestingLauncherState>,
    progress: Option<ProgressState>,
    rdp: Option<Box<RdpState>>,
    popup: Option<PopupState>,
    about: Option<AboutState>,
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
    return_code: ReturnCode,
    initial_state: AppState,
    first_resume: bool,
    next_tick: Option<Instant>,
}

pub fn run_gui(
    catalog: gettext::Catalog,
    initial_state: AppState,
    messages_rx: Receiver<GuiMessage>,
    stop: Trigger,
    fps_limit: Option<u32>,
) -> Result<ReturnCode> {
    let (keys_tx, keys_rx) = bounded::<RawKey>(1024);
    let processing_events = Arc::new(AtomicBool::new(false));
    let event_loop = EventLoop::new()?;

    let mut app = AppHandler {
        catalog,
        testing_launcher: None,
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
        return_code: ReturnCode::Exit,
        initial_state,
        first_resume: true,
        next_tick: None,
    };
    event_loop.run_app(&mut app)?;
    Ok(app.return_code)
}

impl AppHandler {
    pub fn gettext(&self, msgid: &str) -> String {
        self.catalog.gettext(msgid).to_string()
    }

    fn register_window(&mut self, wid: WindowId, kind: WindowKind) {
        self.windows.insert(wid, kind);
    }
    fn unregister_window(&mut self, wid: WindowId) {
        self.windows.remove(&wid);
    }

    fn tick_animations(&mut self) {
        if let Some(ref mut p) = self.progress {
            p.animation_time += 0.3;
            if self.testing_launcher.is_some() && p.pct < 100 {
                p.pct += 1;
                if p.pct >= 100 {
                    p.phase = ProgressPhase::Connected;
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

// ── ApplicationHandler ──────

impl ApplicationHandler for AppHandler {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        el.set_control_flow(ControlFlow::Poll);
        monitor::populate(el);
        if self.first_resume {
            self.first_resume = false;
            match self.initial_state.clone() {
                AppState::Test => {
                    #[cfg(feature = "gui-tester")]
                    {
                        let _ = self.open_testing_launcher(el);
                    }
                }
                AppState::Progress => {
                    let _ = self.open_progress(el);
                }
            }
        }
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
                    Some(WindowKind::TestingLauncher) => {
                        self.handle_testing_launcher_event(el, WindowEvent::RedrawRequested)
                    }
                    Some(WindowKind::Progress) => {
                        self.handle_progress_event(el, WindowEvent::RedrawRequested)
                    }
                    Some(WindowKind::Rdp)
                        if self
                            .rdp
                            .as_ref()
                            .is_some_and(|s| matches!(s.mode, RdpMode::Desktop { .. })) =>
                    {
                        let _ = self.rdp.as_mut().map(|s| s.update_screen());
                    }
                    Some(WindowKind::Rdp)
                        if self
                            .rdp
                            .as_ref()
                            .is_some_and(|s| matches!(s.mode, RdpMode::Rail(_))) =>
                    {
                        self.handle_rail_control_redraw();
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
                    Some(WindowKind::TestingLauncher) => {
                        self.handle_testing_launcher_event(el, event)
                    }
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

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.stop.is_triggered() {
            el.exit();
            return;
        }

        self.process_gui_messages(el);
        self.process_rdp_updates(el);
        self.process_rail_actions(el);

        let has_active_animations = self.progress.is_some() || self.about.is_some();
        let rdp_active = self.rdp.is_some();

        if has_active_animations || rdp_active {
            let now = Instant::now();
            let fps = self.fps_limit;
            let interval = Duration::from_secs_f64(1.0 / fps as f64);

            let mut next_tick = self.next_tick.unwrap_or(now);

            if now >= next_tick {
                self.tick_animations();

                let next = next_tick + interval;
                if next < now {
                    next_tick = now + interval;
                } else {
                    next_tick = next;
                }
            }

            self.next_tick = Some(next_tick);
            el.set_control_flow(ControlFlow::WaitUntil(next_tick));
        } else {
            self.next_tick = None;
            el.set_control_flow(ControlFlow::Wait);
        }
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.stop.trigger();
    }
}
