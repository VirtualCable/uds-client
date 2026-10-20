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
                    let settings = *settings;
                    let needs_credentials =
                        settings.password.is_empty() || settings.user.is_empty();
                    if needs_credentials {
                        if let Err(e) =
                            self.prompt_credentials(el, settings, "Authentication required")
                        {
                            log::error!("Failed to open credentials window: {e}");
                            self.stop.trigger();
                            el.exit();
                        }
                    } else if let Err(e) = self.open_rdp(el, settings) {
                        self.handle_rdp_open_failure(el, &e.to_string());
                    }
                }
            }
        }

        #[cfg(feature = "gui-tester")]
        self.process_testing_launcher_actions(el);
    }

    pub(crate) fn handle_credentials_event(
        &mut self,
        el: &ActiveEventLoop,
        event: winit::event::WindowEvent,
    ) {
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
                        crate::windows::credentials::CredentialsAction::Connect {
                            user,
                            password,
                        } => {
                            state.settings.user = user;
                            state.settings.password = password.to_string();
                            if let Err(e) = self.open_rdp(el, state.settings.clone()) {
                                log::warn!("Failed to enter RDP after credentials ({e})");
                                self.show_error_popup(el, Self::failure_message(&e.to_string()));
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

    /// Open the credentials window, replacing any previous attempt. The
    /// `error_message` is shown as the validation hint; pass an empty string to
    /// display no error.
    pub(crate) fn prompt_credentials(
        &mut self,
        el: &ActiveEventLoop,
        mut settings: rdp::settings::RdpSettings,
        error_message: &str,
    ) -> anyhow::Result<()> {
        settings.password = String::new();
        match crate::windows::credentials::CredentialsState::new(el, settings) {
            Ok(mut c) => {
                if !error_message.is_empty() {
                    c.error_message = Some(error_message.to_string());
                }
                let wid = c.window.id();
                self.register_window(wid, WindowKind::Credentials);
                c.window.set_visible(true);
                c.window.request_redraw();
                self.credentials = Some(c);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Closing an error popup stops the launcher (see `handle_popup_event`).
    pub(crate) fn show_error_popup(&mut self, el: &ActiveEventLoop, msg: String) {
        match PopupState::new(el, PopupKind::Error(msg)) {
            Ok(p) => {
                let wid = p.window.id();
                self.register_window(wid, WindowKind::Popup);
                p.window.set_visible(true);
                p.window.request_redraw();
                self.popup = Some(p);
            }
            Err(e) => {
                log::error!("Failed to open error popup: {e}");
                self.stop.trigger();
                el.exit();
            }
        }
    }

    /// Only one connection attempt is made: with a tunnel, the first attempt
    /// consumes the one-use ticket, so retrying from here could never succeed.
    /// The user has to launch the service again from UDS.
    fn handle_rdp_open_failure(&mut self, el: &ActiveEventLoop, err: &str) {
        log::error!("RDP connection failed: {err}");
        self.show_error_popup(el, Self::failure_message(err));
    }

    fn is_credential_error(err: &str) -> bool {
        let lower = err.to_ascii_lowercase();
        lower.contains("invalid credentials")
            || lower.contains("wrong password")
            || lower.contains("logon failure")
            || lower.contains("password has expired")
            || lower.contains("password must change")
    }

    pub(crate) fn failure_message(err: &str) -> String {
        if err.trim().is_empty() {
            "Could not establish the RDP session. Check your credentials and launch the service again."
                .to_string()
        } else if Self::is_credential_error(err) {
            "Invalid username or password. Launch the service again to retry.".to_string()
        } else {
            format!("Could not connect to the RDP service.\n\n{err}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppHandler;

    #[test]
    fn credential_errors_are_reported_as_credentials() {
        for err in [
            "ERRCONNECT_LOGON_FAILURE: Logon failure",
            "Wrong password",
            "Password has expired",
        ] {
            assert!(AppHandler::failure_message(err).starts_with("Invalid username or password"));
        }
    }

    #[test]
    fn network_errors_are_not_reported_as_credentials() {
        let msg = AppHandler::failure_message("DNS lookup failed for host");
        assert!(msg.starts_with("Could not connect to the RDP service."));
        assert!(msg.contains("DNS lookup failed"));
    }

    #[test]
    fn empty_error_asks_to_relaunch() {
        assert!(AppHandler::failure_message("").contains("launch the service again"));
    }
}
