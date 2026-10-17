// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use shared::log;
use winit::event_loop::ActiveEventLoop;

use crate::AppHandler;
use crate::WindowKind;
use crate::types::GuiMessage;
use crate::windows::popup::{PopupKind, PopupState};
use crate::windows::progress::ProgressPhase;

impl AppHandler {
    pub(crate) fn process_gui_messages(&mut self, el: &ActiveEventLoop) {
        while let Ok(msg) = self.gui_messages_rx.try_recv() {
            match msg {
                GuiMessage::Close => {
                    self.stop.trigger();
                    el.exit();
                    return;
                }
                GuiMessage::CloseProgress => {
                    self.close_progress();
                }
                GuiMessage::ShowError(err) => {
                    if let Ok(p) = PopupState::new(el, PopupKind::Error(err)) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Popup);
                        p.window.set_visible(true);
                        p.window.request_redraw();
                        self.popup = Some(p);
                    }
                }
                GuiMessage::ShowWarning(msg) => {
                    if let Ok(p) = PopupState::new(el, PopupKind::Warning(msg)) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Popup);
                        p.window.set_visible(true);
                        p.window.request_redraw();
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
                        p.window.set_visible(true);
                        p.window.request_redraw();
                        self.popup = Some(p);
                    }
                }
                GuiMessage::ShowProgress => {
                    let _ = self.open_progress(el);
                }
                GuiMessage::Progress(val, msg) => {
                    if let Some(ref mut p) = self.progress {
                        p.pct = val;
                        p.message = msg;
                        if val >= 100 {
                            p.phase = ProgressPhase::Connected;
                        }
                        p.window.request_redraw();
                    }
                }
                GuiMessage::ConnectRdp(settings) => {
                    let needs_credentials = settings.password.is_empty();
                    if needs_credentials {
                        match crate::windows::credentials::CredentialsState::new(el, *settings) {
                            Ok(c) => {
                                let wid = c.window.id();
                                self.register_window(wid, WindowKind::Credentials);
                                c.window.set_visible(true);
                                c.window.request_redraw();
                                self.credentials = Some(c);
                            }
                            Err(e) => {
                                log::error!("Failed to open credentials window: {e}");
                                self.stop.trigger();
                                el.exit();
                            }
                        }
                    } else if let Err(e) = self.open_rdp(el, *settings) {
                        log::error!("Failed to enter RDP: {e}");
                        self.stop.trigger();
                        el.exit();
                        return;
                    }
                }
            }
        }

        #[cfg(feature = "gui-tester")]
        self.process_testing_launcher_actions(el);
    }

    pub(crate) fn handle_credentials_event(&mut self, el: &ActiveEventLoop, event: winit::event::WindowEvent) {
        let Some(ref mut creds) = self.credentials else {
            return;
        };

        match event {
            winit::event::WindowEvent::CloseRequested => {
                let wid = creds.window.id();
                self.unregister_window(wid);
                self.credentials = None;
                self.stop.trigger();
                el.exit();
            }
            winit::event::WindowEvent::Resized(size) => {
                creds.resize(size.width, size.height);
                creds.window.request_redraw();
            }
            winit::event::WindowEvent::RedrawRequested => {
                creds.paint();
            }
            _ => {
                if let Some(action) = creds.handle_event(&event) {
                    let wid = creds.window.id();
                    self.unregister_window(wid);
                    let mut state = self.credentials.take().unwrap();
                    match action {
                        crate::windows::credentials::CredentialsAction::Connect { user, password } => {
                            state.settings.user = user;
                            state.settings.password = password;
                            if let Err(e) = self.open_rdp(el, state.settings) {
                                log::error!("Failed to enter RDP after credentials: {e}");
                                self.stop.trigger();
                                el.exit();
                            }
                        }
                        crate::windows::credentials::CredentialsAction::Cancel => {
                            self.stop.trigger();
                            el.exit();
                        }
                    }
                }
            }
        }
    }
}
