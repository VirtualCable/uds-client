use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;

use anyhow::Result;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

use shared::log;

use super::{AppHandler, WindowKind};
use crate::keymap;
#[cfg(feature = "test-ui")]
use crate::launcher::LaunchAction;
use crate::launcher::{LauncherInner, ProgressPhase};
use crate::logo;
use crate::monitor;
use crate::popup::{PopupKind, PopupState};
use crate::rdp::{
    RailAction, RailWindow, RdpActionResult, RdpState, RdpWindow, handle_rdp_message,
};
use crate::types::{GuiMessage, ReturnCode};
use crate::wgpu_render::WgpuRenderer;

impl AppHandler {
    pub(crate) fn open_rdp(
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
            let server_info = settings.server_info.clone();
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

            // If this is a RAIL session with a server config, start IPC listener
            if let Some(ref srv) = server_info
                && let Some(ref state) = self.rdp
            {
                let cmd_tx = state.command_tx.clone();
                let cmd_ev = state.command_event;
                self.rail_ipc = crate::ipc::bind(&srv.id, &srv.token, move |msg| {
                    let _ = cmd_tx.send(rdp_ffi::commands::RdpCommand::LaunchRailApp {
                        app: msg.rail_app.clone(),
                        args: msg.rail_args.clone(),
                        dir: msg.rail_working_dir.clone(),
                    });
                    unsafe {
                        rdp_ffi::sys::SetEvent(cmd_ev.as_handle());
                    }
                })
                .ok();
            }
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

        Ok(())
    }

    pub(crate) fn close_rdp(&mut self) {
        self.processing_events.store(false, Ordering::Relaxed);
        if let Some(ref r) = self.rdp {
            self.unregister_window(r.window.window.id());
        }
        self.rdp = None;
        self.rail_ipc = None;
    }

    pub(crate) fn process_rail_actions(&mut self, el: &ActiveEventLoop) {
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
                    // Apply any buffered icon for this window
                    if let Some((rgba, w, h)) = state.pending_icons.remove(id)
                        && let Ok(icon) = winit::window::Icon::from_rgba(rgba, w, h)
                        && let Some(rw) = state.rail_windows.get(id)
                    {
                        rw.window.set_window_icon(Some(icon));
                    }

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

    pub(crate) fn process_gui_messages(&mut self, el: &ActiveEventLoop) {
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
                            start: Instant::now(),
                            progress_duration_secs: 30,
                            phase: ProgressPhase::Connecting,
                            auto_animate: false,
                        };
                        if let Some(w) = &l.window {
                            w.set_visible(true);
                            w.request_redraw();
                        }
                    }
                }
                GuiMessage::Progress(val, msg) => {
                    if let Some(ref mut l) = self.launcher {
                        let done = val >= 100;
                        if let LauncherInner::Progress {
                            ref mut pct,
                            ref mut message,
                            ref mut phase,
                            ..
                        } = l.inner
                        {
                            *pct = val;
                            *message = msg;
                            if done {
                                *phase = ProgressPhase::Connected;
                            }
                        }
                        if let Some(w) = &l.window {
                            w.request_redraw();
                        }
                    }
                }
                GuiMessage::ConnectRdp(settings) => {
                    self.close_launcher();
                    if let Err(e) = self.open_rdp(el, *settings) {
                        log::error!("Failed to enter RDP: {e}");
                        self.stop.trigger();
                        el.exit();
                        return;
                    }
                }
            }
        }

        // Launcher test actions
        #[cfg(feature = "test-ui")]
        if let Some(ref mut launcher) = self.launcher
            && let Some(action) = launcher.inner.take_request()
        {
            match action {
                LaunchAction::ShowProgress => {
                    launcher.inner = LauncherInner::Progress {
                        pct: 0,
                        message: String::new(),
                        start: Instant::now(),
                        progress_duration_secs: 5,
                        phase: ProgressPhase::Connecting,
                        auto_animate: true,
                    }
                }
                LaunchAction::GoInvisible => launcher.inner = LauncherInner::Invisible,
                LaunchAction::ShowWarning => {
                    launcher.inner = LauncherInner::Warning("This is a warning message.".into())
                }
                LaunchAction::ShowError => {
                    launcher.inner = LauncherInner::Error("This is an error message.".into())
                }
                LaunchAction::ShowYesNo => {
                    let (rtx, _) = tokio::sync::oneshot::channel::<bool>();
                    launcher.inner = LauncherInner::YesNo {
                        message: "Do you want to continue?".into(),
                        response: Arc::new(std::sync::RwLock::new(Some(rtx))),
                    };
                }
                LaunchAction::ConnectRdp | LaunchAction::ConnectRail => {
                    let is_rail = matches!(action, LaunchAction::ConnectRail);
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
                        server_info: if is_rail {
                            Some(rdp_ffi::settings::ServerInfo {
                                id: "test-uds-rail".to_string(),
                                token: "test-token".to_string(),
                            })
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
                    }
                    // Test: after 4s, send notepad.exe via IPC to the same RAIL session
                    if is_rail {
                        std::thread::spawn(|| {
                            std::thread::sleep(std::time::Duration::from_secs(4));
                            let msg = crate::ipc::RailLaunchMsg {
                                rail_app: "c:\\windows\\notepad.exe".to_string(),
                                rail_args: String::new(),
                                rail_working_dir: String::new(),
                                server_token: "test-token".to_string(),
                            };
                            let ok = crate::ipc::try_send("test-uds-rail", &msg);
                            log::info!("IPC test: sent notepad.exe via IPC → {ok}");
                        });
                    }
                }
            }
        }
    }

    pub(crate) fn process_rdp_updates(&mut self, el: &ActiveEventLoop) {
        let Some(ref mut state) = self.rdp else {
            return;
        };
        while let Ok(message) = state.update_rx.try_recv() {
            match handle_rdp_message(state, message) {
                RdpActionResult::Continue if !state.is_rail => {
                    state.window.window.request_redraw();
                }

                RdpActionResult::Disconnect => {
                    self.stop.trigger();
                    self.return_code = ReturnCode::Exit;
                    self.close_rdp();
                    el.exit();
                    return;
                }
                RdpActionResult::Error(_) => {
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
