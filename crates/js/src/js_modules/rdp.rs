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
    Context, JsResult, JsValue,
    error::{JsError, JsNativeError},
    value::TryFromJs,
};

use rdp::{geom::ScreenSize, settings};

use crate::gui::{GuiMessage, send_message};

#[derive(Debug, TryFromJs)]
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
    pub clipboard_redirection: Option<bool>,
    pub audio_redirection: Option<bool>,
    pub microphone_redirection: Option<bool>,
    pub printer_redirection: Option<bool>,
    pub drives_to_redirect: Option<Vec<String>>,
    pub sound_latency_threshold: Option<u16>,
    pub best_experience: Option<bool>,
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
            clipboard_redirection: None,
            audio_redirection: None,
            microphone_redirection: None,
            printer_redirection: None,
            drives_to_redirect: None,
            sound_latency_threshold: None,
            best_experience: None,
        }
    }
}

impl RdpSettings {
    pub fn is_valid(&self) -> bool {
        !self.server.is_empty()
    }
}

fn start_rdp_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let rdp_settings = extract_js_args!(args, ctx, RdpSettings);
    shared::log::debug!("RDP settings from JS: {:?}", rdp_settings);
    if !rdp_settings.is_valid() {
        return Err(JsError::from_native(
            JsNativeError::error().with_message("Invalid RDP settings: 'server' is required"),
        ));
    }

    // If both screen_width and screen_height are provided, use them. If either is 0, treat as full screen.
    let screen_size = if let (Some(width), Some(height)) =
        (rdp_settings.screen_width, rdp_settings.screen_height)
    {
        if width == 0 || height == 0 {
            ScreenSize::Full
        } else {
            ScreenSize::Fixed(width, height)
        }
    } else {
        ScreenSize::Full
    };

    // Generate Settings from our rdp_settings (defaults match `rdp::settings::RdpSettings` defaults)
    let settings = settings::RdpSettings {
        server: rdp_settings.server,
        port: rdp_settings.port.unwrap_or(3389),
        user: rdp_settings.user.unwrap_or_default(),
        password: rdp_settings.password.unwrap_or_default(),
        domain: rdp_settings.domain.unwrap_or_default(),
        // Default to false to match the core RDP settings defaults
        verify_cert: rdp_settings.verify_cert.unwrap_or(false),
        use_nla: rdp_settings.use_nla.unwrap_or(false),
        screen_size,
        clipboard_redirection: rdp_settings.clipboard_redirection.unwrap_or(true),
        audio_redirection: rdp_settings.audio_redirection.unwrap_or(true),
        microphone_redirection: rdp_settings.microphone_redirection.unwrap_or(false),
        printer_redirection: rdp_settings.printer_redirection.unwrap_or(false),
        drives_to_redirect: rdp_settings
            .drives_to_redirect
            .unwrap_or_else(|| vec!["all".to_string()]),
        sound_latency_threshold: rdp_settings.sound_latency_threshold,
        best_experience: rdp_settings.best_experience.unwrap_or(true),
    };

    send_message(GuiMessage::ConnectRdp(settings));
    // Launcher needs to know that RDP client is running
    // so it doesn't close the GUI immediately
    shared::tasks::mark_internal_rdp_as_running();

    Ok(JsValue::undefined())
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "RDP",
        // Sync functions
        [("start", start_rdp_fn, 1)],
        // Async functions, none here
        [],
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_context, exec_script};
    use anyhow::Result;
    use crossbeam::channel::{Receiver, Sender, bounded};
    use shared::log;

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_init_ctx() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let (messages_tx, messages_rx): (
            Sender<gui::window::types::GuiMessage>,
            Receiver<gui::window::types::GuiMessage>,
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
                drives_to_redirect: ["C", "D"]
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
                    assert_eq!(settings.drives_to_redirect, vec!["C", "D"]);
                    // Defaults defined in `rdp::settings::RdpSettings`
                    assert!(settings.clipboard_redirection);
                    assert!(settings.audio_redirection);
                    assert!(!settings.microphone_redirection);
                    assert!(!settings.printer_redirection);
                    assert_eq!(settings.sound_latency_threshold, None);
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
        log::setup_logging("debug", log::LogType::Tests);
        let (messages_tx, messages_rx): (
            Sender<gui::window::types::GuiMessage>,
            Receiver<gui::window::types::GuiMessage>,
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
                    assert!(!settings.use_nla);
                    match settings.screen_size {
                        ScreenSize::Fixed(w, h) => {
                            assert_eq!(w, 1024);
                            assert_eq!(h, 768);
                        }
                        _ => panic!("Expected fixed screen size"),
                    }
                    assert_eq!(settings.drives_to_redirect, vec!["all"]);
                    assert!(settings.clipboard_redirection);
                    assert!(settings.audio_redirection);
                    assert!(!settings.microphone_redirection);
                    assert!(!settings.printer_redirection);
                    assert_eq!(settings.sound_latency_threshold, None);
                }
                _ => panic!("Expected GuiMessage::ConnectRdp"),
            },
            Err(e) => {
                panic!("Expected a GuiMessage but none was sent: {}", e);
            }
        }

        Ok(())
    }
}
