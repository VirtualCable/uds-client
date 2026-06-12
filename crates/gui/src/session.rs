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
use crate::windows::progress::{ProgressPhase, ProgressState};

impl AppHandler {
    pub(crate) fn process_gui_messages(&mut self, el: &ActiveEventLoop) {
        macro_rules! tr {
            ($msg:expr) => {
                self.gettext($msg)
            };
        }
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
                    if let Ok(p) = ProgressState::new(
                        el,
                        tr!("UDS Launcher"),
                        tr!("CANCEL"),
                        tr!("Connecting to RDP server..."),
                        tr!("Connected."),
                    ) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Progress);
                        self.progress = Some(p);
                    }
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
        if let Some(ref mut launcher) = self.launcher
            && let Some(action) = launcher.inner.take_request()
        {
            use crate::windows::launcher::LaunchAction;
            match action {
                LaunchAction::ShowProgress => {
                    if let Ok(p) = ProgressState::new(
                        el,
                        tr!("UDS Launcher"),
                        tr!("CANCEL"),
                        tr!("Connecting to RDP server..."),
                        tr!("Connected."),
                    ) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Progress);
                        self.progress = Some(p);
                    }
                }
                LaunchAction::ShowAbout => {
                    if let Ok(a) = crate::windows::about::AboutState::new(el) {
                        let wid = a.window().id();
                        self.register_window(wid, WindowKind::About);
                        self.about = Some(a);
                    }
                }
                LaunchAction::ShowWarning => {
                    if let Ok(p) = PopupState::new(
                        el,
                        PopupKind::Warning("This is a test warning message.".into()),
                    ) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Popup);
                        self.popup = Some(p);
                    }
                }
                LaunchAction::ShowError => {
                    if let Ok(p) = PopupState::new(
                        el,
                        PopupKind::Error("This is a test error message.".into()),
                    ) {
                        let wid = p.window.id();
                        self.register_window(wid, WindowKind::Popup);
                        self.popup = Some(p);
                    }
                }
                LaunchAction::ShowYesNo => {
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
                LaunchAction::ConnectRdp | LaunchAction::ConnectRail => {
                    let is_rail = matches!(action, LaunchAction::ConnectRail);
                    let settings = rdp_ffi::settings::RdpSettings {
                        server: "172.27.247.161".to_string(),
                        user: "user".to_string(),
                        password: "temporal".to_string(),
                        screen_size: rdp_ffi::geom::ScreenSize::Fixed(800, 600),
                        microphone_redirection: true,
                        webcam: Some(rdp_ffi::settings::WebcamSettings {
                            enabled: true,
                            quality: 80,
                            fps: 15,
                            ..rdp_ffi::settings::WebcamSettings::default()
                        }),
                        best_experience: true,
                        use_local_scaler: true,
                        rail: if is_rail {
                            Some(rdp_ffi::settings::RailSettings {
                                app: "||win32calc".to_string(),
                                args: None,
                                working_dir: None,
                                title: Some("Ms Paint UDS App".to_string()),
                                server_info: Some(rdp_ffi::settings::ServerInfo {
                                    id: "test-uds-rail".to_string(),
                                    token: "test-token".to_string(),
                                }),
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
                }
                #[cfg(not(feature = "gui-tester"))]
                LaunchAction::ConnectRdp | LaunchAction::ConnectRail => {
                    log::warn!(
                        "RDP Connect/RAIL test actions are disabled without 'gui-tester' feature"
                    );
                }
            }
        }
    }
}
