// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

#![allow(dead_code)]
use anyhow::Result;

use boa_engine::{
    Context, JsResult, JsValue,
    error::{JsError, JsNativeError},
    value::TryFromJs,
};

use shared::log;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::gui::{GuiMessage, send_message};
use gui::types::{UrpRedirections, UrpSettings};

/// Default URP agent port.
const DEFAULT_PORT: u16 = 14433;

#[derive(Debug, Default, TryFromJs, Zeroize, ZeroizeOnDrop, Clone)]
struct JsUrpRedirections {
    pub audio: Option<bool>,
    pub mic: Option<bool>,
    pub clipboard: Option<bool>,
    pub printing: Option<bool>,
    pub webcam: Option<bool>,
    pub smartcard: Option<bool>,
    pub storage: Option<bool>,
}

#[derive(Debug, Default, TryFromJs, Zeroize, ZeroizeOnDrop, Clone)]
struct JsUrpOptions {
    pub preferred_codec: Option<String>,
}

#[derive(Debug, TryFromJs, Zeroize, ZeroizeOnDrop)]
struct JsUrpSettings {
    #[zeroize(skip)]
    pub server: String,
    pub port: Option<u16>,
    /// Broker ticket (48 alphanumeric chars, verbatim — the pq wire
    /// mapping is raw UTF-8; SHA-384 is only the standalone/lab mode).
    pub ticket: String,
    /// Session PSK as 64 hex chars (the KEM envelope's shared secret).
    pub shared_secret: String,
    /// Interactive logon credentials (the script asked the user when the
    /// broker did not know them).
    pub user: Option<String>,
    pub password: Option<String>,
    pub domain: Option<String>,
    #[zeroize(skip)]
    pub screen_width: Option<u32>,
    #[zeroize(skip)]
    pub screen_height: Option<u32>,
    #[zeroize(skip)]
    pub fullscreen: Option<bool>,
    #[zeroize(skip)]
    pub redirections: Option<JsUrpRedirections>,
    #[zeroize(skip)]
    pub options: Option<JsUrpOptions>,
}

impl Default for JsUrpSettings {
    fn default() -> Self {
        JsUrpSettings {
            server: String::new(),
            port: Some(DEFAULT_PORT),
            ticket: String::new(),
            shared_secret: String::new(),
            user: None,
            password: None,
            domain: None,
            screen_width: None,
            screen_height: None,
            fullscreen: None,
            redirections: None,
            options: None,
        }
    }
}

impl JsUrpSettings {
    fn to_core_settings(&self) -> Result<UrpSettings, String> {
        if self.server.is_empty() {
            return Err("'server' is required".into());
        }
        if self.ticket.len() != 48 || !self.ticket.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err("'ticket' must be 48 alphanumeric characters".into());
        }
        let ticket = uds_rp::core::wire_ticket_from_str(&self.ticket)
            .map_err(|e| format!("invalid ticket: {e}"))?;
        let key: [u8; 32] = uds_rp::core::hex::decode(&self.shared_secret)
            .map_err(|_| "'shared_secret' must be 64 hex characters".to_string())?;
        // Credentials travel together or not at all.
        if self.user.is_some() != self.password.is_some() {
            return Err("'user' and 'password' must come together".into());
        }
        let red = self.redirections.clone().unwrap_or_default();
        let defs = UrpRedirections::default();
        let opt = self.options.clone().unwrap_or_default();
        Ok(UrpSettings {
            server: self.server.clone(),
            port: self.port.unwrap_or(DEFAULT_PORT),
            ticket,
            key,
            user: self.user.clone(),
            password: self.password.clone(),
            domain: self.domain.clone(),
            screen_width: self.screen_width.unwrap_or(0),
            screen_height: self.screen_height.unwrap_or(0),
            fullscreen: self.fullscreen.unwrap_or(false),
            redirections: UrpRedirections {
                audio: red.audio.unwrap_or(defs.audio),
                mic: red.mic.unwrap_or(defs.mic),
                clipboard: red.clipboard.unwrap_or(defs.clipboard),
                printing: red.printing.unwrap_or(defs.printing),
                webcam: red.webcam.unwrap_or(defs.webcam),
                smartcard: red.smartcard.unwrap_or(defs.smartcard),
                storage: red.storage.unwrap_or(defs.storage),
            },
            preferred_codec: opt.preferred_codec.clone(),
        })
    }
}

fn start_urp_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let js_settings = extract_js_args!(args, ctx, JsUrpSettings);
    log::debug!(
        "URP settings parsed - server: '{}', port: {:?}, fullscreen: {:?}",
        js_settings.server,
        js_settings.port,
        js_settings.fullscreen
    );

    let settings = js_settings
        .to_core_settings()
        .map_err(|e| JsError::from_native(JsNativeError::error().with_message(e)))?;

    log::debug!("Starting URP with settings: {:?}", settings);

    send_message(GuiMessage::ConnectUrp(Box::new(settings)));
    // Launcher needs to know that a remote session is running
    // so it doesn't close the GUI immediately
    connection::tasks::mark_remote_session_running();

    Ok(JsValue::undefined())
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "URP",
        // Sync functions
        [("start", start_urp_fn, 1),],
        // Async functions
        [],
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

    fn test_setup() -> Result<Receiver<gui::types::GuiMessage>> {
        log::setup_logging("debug", log::LogType::Test);
        let (messages_tx, messages_rx): (
            Sender<gui::types::GuiMessage>,
            Receiver<gui::types::GuiMessage>,
        ) = bounded(32);
        crate::gui::set_sender(messages_tx);
        Ok(messages_rx)
    }

    const VALID_TICKET: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRST01";
    const VALID_SECRET: &str = "733b5b19754ebb4af75efa1b885e078116b85abb8488c4381078bf5fcb90958c";

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_start_minimal() -> Result<()> {
        let messages_rx = test_setup()?;
        let mut ctx = create_context(None)?;
        register(&mut ctx)?;
        let script = format!(
            r#"
            URP.start({{
                server: "172.27.1.4",
                ticket: "{VALID_TICKET}",
                shared_secret: "{VALID_SECRET}"
            }});
        "#
        );
        exec_script(&mut ctx, &script)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        match messages_rx.try_recv() {
            Ok(GuiMessage::ConnectUrp(settings)) => {
                assert_eq!(settings.server, "172.27.1.4");
                assert_eq!(settings.port, DEFAULT_PORT);
                // pq wire mapping: raw UTF-8, NUL-padded (no hashing).
                assert_eq!(&settings.ticket[..48], VALID_TICKET.as_bytes());
                assert_eq!(settings.key[0], 0x73);
                assert_eq!(settings.key[31], 0x8c);
                assert!(settings.user.is_none());
                assert!(settings.password.is_none());
                assert!(!settings.fullscreen);
                assert_eq!((settings.screen_width, settings.screen_height), (0, 0));
                // Conservative defaults (broker channel policy).
                assert!(settings.redirections.audio);
                assert!(settings.redirections.clipboard);
                assert!(settings.redirections.storage);
                assert!(!settings.redirections.mic);
                assert!(!settings.redirections.printing);
                assert!(!settings.redirections.webcam);
                assert!(!settings.redirections.smartcard);
                assert!(settings.preferred_codec.is_none());
            }
            other => panic!("Expected GuiMessage::ConnectUrp, got {:?}", other.is_ok()),
        }
        Ok(())
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_start_full() -> Result<()> {
        let messages_rx = test_setup()?;
        let mut ctx = create_context(None)?;
        register(&mut ctx)?;
        let script = format!(
            r#"
            URP.start({{
                server: "broker-vm",
                port: 14434,
                ticket: "{VALID_TICKET}",
                shared_secret: "{VALID_SECRET}",
                user: "dkmaster",
                password: "s3cret",
                domain: "LAB",
                screen_width: 2560,
                screen_height: 1440,
                fullscreen: true,
                redirections: {{ audio: false, mic: true, webcam: true }},
                options: {{ preferred_codec: "ladder" }}
            }});
        "#
        );
        exec_script(&mut ctx, &script)
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        match messages_rx.try_recv() {
            Ok(GuiMessage::ConnectUrp(settings)) => {
                assert_eq!(settings.server, "broker-vm");
                assert_eq!(settings.port, 14434);
                assert_eq!(settings.user.as_deref(), Some("dkmaster"));
                assert_eq!(settings.password.as_deref(), Some("s3cret"));
                assert_eq!(settings.domain.as_deref(), Some("LAB"));
                assert_eq!(
                    (settings.screen_width, settings.screen_height),
                    (2560, 1440)
                );
                assert!(settings.fullscreen);
                assert!(!settings.redirections.audio);
                assert!(settings.redirections.mic);
                assert!(settings.redirections.webcam);
                // Untouched redirections keep defaults.
                assert!(settings.redirections.clipboard);
                assert!(!settings.redirections.printing);
                assert_eq!(settings.preferred_codec.as_deref(), Some("ladder"));
            }
            _ => panic!("Expected GuiMessage::ConnectUrp"),
        }
        Ok(())
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_rejects_bad_ticket_and_secret() -> Result<()> {
        let messages_rx = test_setup()?;
        let mut ctx = create_context(None)?;
        register(&mut ctx)?;
        for (ticket, secret) in [
            ("short", VALID_SECRET),
            // 48 chars but not alphanumeric
            (
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa!",
                VALID_SECRET,
            ),
            (VALID_TICKET, "not-hex"),
            (VALID_TICKET, "733b"),
        ] {
            let script = format!(
                r#"URP.start({{ server: "h", ticket: "{ticket}", shared_secret: "{secret}" }});"#
            );
            let result = exec_script(&mut ctx, &script).await;
            assert!(result.is_err(), "ticket={ticket} secret={secret} must fail");
        }
        assert!(messages_rx.try_recv().is_err());
        Ok(())
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_rejects_half_credentials() -> Result<()> {
        let messages_rx = test_setup()?;
        let mut ctx = create_context(None)?;
        register(&mut ctx)?;
        let script = format!(
            r#"URP.start({{ server: "h", ticket: "{VALID_TICKET}", shared_secret: "{VALID_SECRET}", user: "u" }});"#
        );
        assert!(exec_script(&mut ctx, &script).await.is_err());
        assert!(messages_rx.try_recv().is_err());
        Ok(())
    }
}
