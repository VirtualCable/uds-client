// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
#![allow(dead_code)]
use anyhow::Result;

use boa_engine::{
    Context, JsResult, JsString, JsValue,
    error::{JsError, JsNativeError},
    value::TryFromJs,
};

use connection::broker;
use rdp::{geom::ScreenSize, settings};
use shared::log;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::gui::{GuiMessage, send_message};

#[derive(Debug, TryFromJs, Zeroize, ZeroizeOnDrop)]
struct ServerInfo {
    #[zeroize(skip)]
    pub id: String,
    pub token: String,
}

#[derive(Debug, TryFromJs, Zeroize, ZeroizeOnDrop)]
struct RailSettings {
    #[zeroize(skip)]
    pub app: String,
    pub args: Option<String>,
    pub working_dir: Option<String>,
    pub title: Option<String>,
    pub server_info: Option<ServerInfo>,
    #[zeroize(skip)]
    pub behavior: Option<String>,
}

#[derive(Debug, TryFromJs, Zeroize, ZeroizeOnDrop, Clone)]
struct WebcamSettings {
    pub enabled: bool,
    pub quality: Option<u32>,
    pub fps: Option<u32>,
    pub size_limit: Option<(u32, u32)>,
}

#[derive(Debug, Default, TryFromJs, Zeroize, ZeroizeOnDrop, Clone)]
struct RdpRedirections {
    pub clipboard: Option<bool>,
    pub audio: Option<bool>,
    pub mic: Option<bool>,
    pub printing: Option<bool>,
    pub drives: Option<Vec<String>>,
    pub webcam: Option<WebcamSettings>,
    pub sound_latency_threshold: Option<u16>,
}

#[derive(Debug, TryFromJs, Zeroize, ZeroizeOnDrop)]
struct RdpSettings {
    pub server: String,
    pub port: Option<u32>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub domain: Option<String>,
    pub verify_cert: Option<bool>,
    pub use_nla: Option<bool>,
    pub screen_width: Option<u32>,
    pub screen_height: Option<u32>,
    pub best_experience: Option<bool>,
    pub redirections: Option<RdpRedirections>,
    pub rail: Option<RailSettings>,
    pub use_local_scaler: Option<bool>,
    pub use_tunnel: Option<bool>,
}

impl Default for RdpSettings {
    fn default() -> Self {
        RdpSettings {
            server: String::new(),
            port: Some(3389),
            user: None,
            password: None,
            domain: None,
            verify_cert: None,
            use_nla: None,
            screen_width: None,
            screen_height: None,
            best_experience: None,
            redirections: None,
            rail: None,
            use_local_scaler: None,
            use_tunnel: None,
        }
    }
}

impl RdpSettings {
    pub fn is_valid(&self) -> bool {
        !self.server.is_empty()
    }

    pub fn to_core_settings(&self) -> settings::RdpSettings {
        let screen_size =
            if let (Some(width), Some(height)) = (self.screen_width, self.screen_height) {
                if width == 0 || height == 0 {
                    ScreenSize::Full
                } else {
                    ScreenSize::Fixed(width, height)
                }
            } else {
                ScreenSize::Full
            };

        let defs = settings::RdpSettings::default();
        let redirections = self.redirections.clone().unwrap_or_default();

        settings::RdpSettings {
            server: self.server.clone(),
            port: self.port.unwrap_or(defs.port),
            user: self.user.clone().unwrap_or(defs.user),
            password: self.password.clone().unwrap_or(defs.password),
            domain: self.domain.clone().unwrap_or(defs.domain),
            verify_cert: self.verify_cert.unwrap_or(defs.verify_cert),
            use_nla: self.use_nla.unwrap_or(defs.use_nla),
            screen_size,
            best_experience: self.best_experience.unwrap_or(defs.best_experience),
            redirections: settings::RdpRedirections {
                clipboard: redirections
                    .clipboard
                    .unwrap_or(defs.redirections.clipboard),
                audio: redirections.audio.unwrap_or(defs.redirections.audio),
                mic: redirections.mic.unwrap_or(defs.redirections.mic),
                printing: redirections.printing.unwrap_or(defs.redirections.printing),
                drives: redirections
                    .drives
                    .clone()
                    .unwrap_or_else(|| defs.redirections.drives.clone()),
                webcam: redirections
                    .webcam
                    .as_ref()
                    .map(|w| settings::WebcamSettings {
                        enabled: w.enabled,
                        quality: w.quality.unwrap_or(80),
                        fps: w.fps.unwrap_or(15),
                        codec: settings::WebcamCodec::Best,
                        browser_h264: false,
                        width: 640,
                        height: 480,
                        size_limit: w.size_limit,
                    }),
                sound_latency_threshold: redirections.sound_latency_threshold,
            },
            rail: self.rail.as_ref().map(|r| settings::RailSettings {
                app: r.app.clone(),
                args: r.args.clone(),
                working_dir: r.working_dir.clone(),
                title: r.title.clone(),
                server_info: r.server_info.as_ref().map(|s| settings::ServerInfo {
                    id: s.id.clone(),
                    token: s.token.clone(),
                }),
                behavior: r
                    .behavior
                    .as_deref()
                    .map(|b| match b.to_lowercase().as_str() {
                        "compositegdi" | "composite" => settings::RailBehavior::CompositeGdi,
                        "individualwindows" | "individual" => {
                            settings::RailBehavior::IndividualWindows
                        }
                        _ => settings::RailBehavior::IndividualWindows,
                    })
                    .unwrap_or(settings::RailBehavior::IndividualWindows),
            }),
            desktop_scale: 1.0,
            use_local_scaler: self.use_local_scaler.unwrap_or(true),
            use_tunnel: self.use_tunnel.unwrap_or(defs.use_tunnel),
        }
    }
}

fn start_rdp_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let rdp_settings = extract_js_args!(args, ctx, RdpSettings);
    log::debug!(
        "RdpSettings parsed - server: '{}', user: {:?}, port: {:?}",
        rdp_settings.server,
        rdp_settings.user,
        rdp_settings.port
    );
    if !rdp_settings.is_valid() {
        return Err(JsError::from_native(
            JsNativeError::error().with_message("Invalid RDP settings: 'server' is required"),
        ));
    }

    let settings = rdp_settings.to_core_settings();

    log::debug!("Starting RDP with settings: {:?}", settings);

    // If we have a server config and a rail_app, try sending via IPC to an existing session
    if let Some(ref rail) = settings.rail
        && let Some(ref srv) = rail.server_info
    {
        let msg = gui::ipc::RailLaunchMsg {
            app: rail.app.clone(),
            args: rail.args.clone().unwrap_or_default(),
            working_dir: rail.working_dir.clone().unwrap_or_default(),
            server_token: srv.token.clone(),
        };
        if gui::ipc::try_send(&srv.id, &msg) {
            log::info!(
                "Sent RAIL app via IPC channel: {} (server_id={})",
                rail.app,
                srv.id
            );
            return Ok(JsValue::undefined());
        }
    }

    send_message(GuiMessage::ConnectRdp(Box::new(settings)));
    // Launcher needs to know that RDP client is running
    // so it doesn't close the GUI immediately
    connection::tasks::mark_internal_rdp_as_running();

    Ok(JsValue::undefined())
}

async fn sign_rdp_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &std::cell::RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let (rdp_string, ticket) = {
        let mut ctx_borrow = ctx.borrow_mut();
        extract_js_args!(args, &mut *ctx_borrow, String, String)
    };
    let api = broker::api::get_api().map_err(|e| {
        JsError::from_native(
            JsNativeError::error().with_message(format!("Failed to get broker API: {}", e)),
        )
    })?;
    let signed_rdp = api
        .request_rdp_sign(&ticket, &rdp_string)
        .await
        .map_err(|e| {
            JsError::from_native(
                JsNativeError::error().with_message(format!("Failed to sign RDP string: {}", e)),
            )
        })?;
    Ok(JsValue::from(JsString::from(signed_rdp)))
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    // Disable format that would make this less readable
    register_js_module!(
        ctx,
        "RDP",
        // Sync functions
        [("start", start_rdp_fn, 1),],
        // Async functions
        [("sign", sign_rdp_fn, 2),],
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_context, exec_script};
    use anyhow::Result;
    use flume::{Receiver, Sender, bounded};
    use shared::log;

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_init_ctx() -> Result<()> {
        log::setup_logging("debug", log::LogType::Test);
        let (messages_tx, messages_rx): (
            Sender<gui::types::GuiMessage>,
            Receiver<gui::types::GuiMessage>,
        ) = bounded(32);

        crate::gui::set_sender(messages_tx);

        let mut ctx = create_context(None)?;
        register(&mut ctx)?;
        let script = r#"
            let rdpSettings = {
                server: "localhost",
                port: 3389,
                user: "testuser",
                password: "password",
                domain: "DOMAIN",
                verify_cert: true,
                use_nla: true,
                screen_width: 1024,
                screen_height: 768,
                redirections: {
                    drives: ["C", "D"]
                }
            };
            RDP.start(rdpSettings);
        "#;
        _ = exec_script(&mut ctx, script).await;
        // Verify that a GuiMessage::ConnectRdp was sent
        match messages_rx.try_recv() {
            Ok(gui_msg) => match gui_msg {
                GuiMessage::ConnectRdp(settings) => {
                    assert_eq!(settings.server, "localhost");
                    assert_eq!(settings.port, 3389);
                    assert_eq!(settings.user, "testuser");
                    assert_eq!(settings.password, "password");
                    assert_eq!(settings.domain, "DOMAIN");
                    assert!(settings.verify_cert);
                    assert!(settings.use_nla);
                    match settings.screen_size {
                        ScreenSize::Fixed(w, h) => {
                            assert_eq!(w, 1024);
                            assert_eq!(h, 768);
                        }
                        _ => panic!("Expected fixed screen size"),
                    }
                    assert_eq!(settings.redirections.drives, vec!["C", "D"]);
                    // Defaults defined in `rdp::settings::RdpSettings`
                    assert!(settings.redirections.clipboard);
                    assert!(settings.redirections.audio);
                    assert!(!settings.redirections.mic);
                    assert!(!settings.redirections.printing);
                    assert_eq!(settings.redirections.sound_latency_threshold, None);
                }
                _ => panic!("Expected GuiMessage::ConnectRdp"),
            },
            Err(e) => {
                panic!("Expected a GuiMessage but none was sent: {}", e);
            }
        }
        Ok(())
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_defaults() -> Result<()> {
        log::setup_logging("debug", log::LogType::Test);
        let (messages_tx, messages_rx): (
            Sender<gui::types::GuiMessage>,
            Receiver<gui::types::GuiMessage>,
        ) = bounded(32);

        crate::gui::set_sender(messages_tx);

        let mut ctx = create_context(None)?;
        register(&mut ctx)?;
        let script = r#"
            let rdpSettings = {
                server: "localhost"
            };
            RDP.start(rdpSettings);
        "#;
        _ = exec_script(&mut ctx, script).await;

        match messages_rx.try_recv() {
            Ok(gui_msg) => match gui_msg {
                GuiMessage::ConnectRdp(settings) => {
                    assert_eq!(settings.server, "localhost");
                    assert_eq!(settings.port, 3389);
                    assert_eq!(settings.user, "");
                    assert_eq!(settings.password, "");
                    assert_eq!(settings.domain, "");
                    assert!(!settings.verify_cert);
                    assert!(settings.use_nla);
                    match settings.screen_size {
                        ScreenSize::Full => {}
                        _ => panic!("Expected full screen size, got {:?}", settings.screen_size),
                    }
                    assert_eq!(settings.redirections.drives, vec!["all"]);
                    assert!(settings.redirections.clipboard);
                    assert!(settings.redirections.audio);
                    assert!(!settings.redirections.mic);
                    assert!(!settings.redirections.printing);
                    assert_eq!(settings.redirections.sound_latency_threshold, None);
                }
                _ => panic!("Expected GuiMessage::ConnectRdp"),
            },
            Err(e) => {
                panic!("Expected a GuiMessage but none was sent: {}", e);
            }
        }

        Ok(())
    }

    #[test]
    fn settings_is_valid_empty() {
        let s = RdpSettings::default();
        assert!(!s.is_valid());
    }

    #[test]
    fn settings_is_valid_nonempty() {
        let mut s = RdpSettings::default();
        s.server = "host".into();
        assert!(s.is_valid());
    }

    #[test]
    fn settings_defaults() {
        let s = RdpSettings::default();
        assert_eq!(s.server, "");
        assert_eq!(s.port, Some(3389));
        assert!(s.user.is_none());
        assert!(s.password.is_none());
        assert!(s.rail.is_none());
    }

    #[test]
    fn to_core_screen_full_when_missing() {
        let s = RdpSettings::default();
        let core = s.to_core_settings();
        assert!(matches!(core.screen_size, ScreenSize::Full));
    }

    #[test]
    fn to_core_screen_full_when_zero() {
        let mut s = RdpSettings::default();
        s.server = "h".into();
        s.screen_width = Some(0);
        s.screen_height = Some(768);
        assert!(matches!(s.to_core_settings().screen_size, ScreenSize::Full));
    }

    #[test]
    fn to_core_screen_fixed() {
        let mut s = RdpSettings::default();
        s.server = "h".into();
        s.screen_width = Some(1024);
        s.screen_height = Some(768);
        assert!(matches!(
            s.to_core_settings().screen_size,
            ScreenSize::Fixed(1024, 768)
        ));
    }

    #[test]
    fn to_core_use_local_scaler_defaults_true() {
        let s = RdpSettings::default();
        assert!(s.to_core_settings().use_local_scaler);
    }

    #[test]
    fn to_core_use_local_scaler_explicit_false() {
        let mut s = RdpSettings::default();
        s.server = "h".into();
        s.use_local_scaler = Some(false);
        assert!(!s.to_core_settings().use_local_scaler);
    }

    #[test]
    fn to_core_server_info_mapping() {
        let mut s = RdpSettings::default();
        s.server = "h".into();
        s.rail = Some(RailSettings {
            app: "myapp".into(),
            args: None,
            working_dir: None,
            title: None,
            server_info: Some(ServerInfo {
                id: "myid".into(),
                token: "mytok".into(),
            }),
            behavior: None,
        });
        let core = s.to_core_settings();
        let si = core.rail.unwrap().server_info.unwrap();
        assert_eq!(si.id, "myid");
        assert_eq!(si.token, "mytok");
    }

    #[test]
    fn to_core_server_info_none() {
        let s = RdpSettings::default();
        assert!(s.to_core_settings().rail.is_none());
    }
}
