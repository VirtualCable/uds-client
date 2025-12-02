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
    pub drives_to_redirect: Vec<String>,
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
            drives_to_redirect: vec![],
        }
    }
}

impl RdpSettings {
    pub fn is_valid(&self) -> bool {
        !self.server.is_empty()
    }
}

async fn start_rdp_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &std::cell::RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let mut ctx_borrow = ctx.borrow_mut();
    let rdp_settings = extract_js_args!(args, &mut ctx_borrow, RdpSettings);
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
        drives_to_redirect: rdp_settings.drives_to_redirect,
    };

    send_message(GuiMessage::ConnectRdp(settings));

    Ok(JsValue::undefined())
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "RDP",
        // Sync functions
        [],
        // Async functions, none here
        [("startRDP", start_rdp_fn, 1)],
    );
    Ok(())
}
