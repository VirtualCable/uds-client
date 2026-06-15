// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

use shared::log;
use std::sync::Arc;
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
                    self.close_testing_launcher();
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
        if let Some(ref mut launcher) = self.testing_launcher
            && let Some(action) = launcher.inner.take_request()
        {
            use crate::windows::testing_launcher_window::TestingLaunchAction;
            match action {
                TestingLaunchAction::ShowProgress => {
                    let _ = self.open_progress(el);
                }
                TestingLaunchAction::ShowAbout => {
                    if let Ok(a) = crate::windows::about::AboutState::new(el) {
                        let wid = a.window().id();
                        self.register_window(wid, WindowKind::About);
                        self.about = Some(a);
                    }
                }
                TestingLaunchAction::ShowWarning => {
                    if let Ok(p) = PopupState::new(
                        el,
                        PopupKind::Warning("This is a test warning message.".into()),
                    ) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Popup);
                        self.popup = Some(p);
                    }
                }
                TestingLaunchAction::ShowError => {
                    if let Ok(p) = PopupState::new(
                        el,
                        PopupKind::Error("This is a test error message.".into()),
                    ) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Popup);
                        self.popup = Some(p);
                    }
                }
                TestingLaunchAction::ShowYesNo => {
                    let (rtx, _) = tokio::sync::oneshot::channel::<bool>();
                    if let Ok(p) = PopupState::new(
                        el,
                        PopupKind::YesNo {
                            message:
                                "This is a test confirmation message. Do you want to continue?"
                                    .into(),
                            response: Arc::new(std::sync::RwLock::new(Some(rtx))),
                        },
                    ) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Popup);
                        self.popup = Some(p);
                    }
                }
                #[cfg(feature = "gui-tester")]
                TestingLaunchAction::ConnectRdp | TestingLaunchAction::ConnectRail => {
                    let is_rail = matches!(action, TestingLaunchAction::ConnectRail);
                    let settings = rdp_ffi::settings::RdpSettings {
                        server: "172.27.247.161".to_string(),
                        user: "user".to_string(),
                        password: "temporal".to_string(),
                        screen_size: rdp_ffi::geom::ScreenSize::Fixed(800, 600),
                        redirections: rdp_ffi::settings::RdpRedirections {
                            clipboard: true,
                            audio: true,
                            mic: true,
                            printing: false,
                            drives: vec!["all".to_string()],
                            webcam: Some(rdp_ffi::settings::WebcamSettings {
                                enabled: true,
                                quality: 80,
                                fps: 15,
                                ..rdp_ffi::settings::WebcamSettings::default()
                            }),
                            sound_latency_threshold: None,
                        },
                        best_experience: true,
                        options: rdp_ffi::settings::RdpOptions {
                            use_local_scaler: true,
                            ..Default::default()
                        },
                        rail: if is_rail {
                            Some(rdp_ffi::settings::RailSettings {
                                app: "c:\\windows\\system32\\calc.exe".to_string(),
                                args: None,
                                working_dir: None,
                                title: Some("Windows Calculator".to_string()),
                                server_info: Some(rdp_ffi::settings::ServerInfo {
                                    id: "test-uds-rail".to_string(),
                                    token: "test-token".to_string(),
                                }),
                                ..Default::default()
                            })
                        } else {
                            None
                        },
                        ..Default::default()
                    };
                    self.close_testing_launcher();
                    if let Err(e) = self.open_rdp(el, settings) {
                        log::error!("Failed to enter RDP: {e}");
                        self.stop.trigger();
                        el.exit();
                    }
                }
                #[cfg(not(feature = "gui-tester"))]
                TestingLaunchAction::ConnectRdp | TestingLaunchAction::ConnectRail => {
                    log::warn!(
                        "RDP Connect/RAIL test actions are disabled without 'gui-tester' feature"
                    );
                }
            }
        }
    }
}
