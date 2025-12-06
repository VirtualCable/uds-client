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
    pub drives_to_redirect: Option<Vec<String>>,
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
            drives_to_redirect: None,
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

    // If screensize width is 0 or height is 0, we can assume full screen
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

    // Generate Settings from our rdp_settings
    let settings = settings::RdpSettings {
        server: rdp_settings.server,
        port: rdp_settings.port.unwrap_or(3389),
        user: rdp_settings.user.unwrap_or_default(),
        password: rdp_settings.password.unwrap_or_default(),
        domain: rdp_settings.domain.unwrap_or_default(),
        verify_cert: rdp_settings.verify_cert.unwrap_or(true),
        use_nla: rdp_settings.use_nla.unwrap_or(true),
        screen_size,
        clipboard_redirection: rdp_settings.clipboard_redirection.unwrap_or(true),
        drives_to_redirect: rdp_settings.drives_to_redirect.unwrap_or_default(),
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
