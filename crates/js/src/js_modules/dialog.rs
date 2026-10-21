// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::{Arc, RwLock};

use anyhow::Result;
use boa_engine::{
    Context, JsResult, JsValue,
    error::{JsError, JsNativeError},
};
use tokio::sync::oneshot;

use gui::types::{
    DialogButton, DialogField, DialogFieldType, DialogResult, DialogSpec, GuiMessage,
};
use gui::windows::credentials::CredentialsState;

use crate::gui::send_message;

async fn show_dialog_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &std::cell::RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let spec: DialogSpec = {
        let mut ctx_borrow = ctx.borrow_mut();
        let arg = args.first().ok_or_else(|| {
            JsError::from_native(
                JsNativeError::typ().with_message("Dialog.show expects a dialog specification"),
            )
        })?;

        if let Some(spec_str) = arg.as_string() {
            serde_json::from_str(&spec_str.to_std_string_escaped()).map_err(|e| {
                JsError::from_native(
                    JsNativeError::typ()
                        .with_message(format!("Failed to parse DialogSpec from string: {}", e)),
                )
            })?
        } else {
            let json_val = arg
                .to_json(&mut ctx_borrow)
                .map_err(|e| {
                    JsError::from_native(
                        JsNativeError::typ()
                            .with_message(format!("Failed to convert argument to JSON: {}", e)),
                    )
                })?
                .ok_or_else(|| {
                    JsError::from_native(
                        JsNativeError::typ()
                            .with_message("Dialog specification cannot be undefined"),
                    )
                })?;
            serde_json::from_value(json_val).map_err(|e| {
                JsError::from_native(
                    JsNativeError::typ()
                        .with_message(format!("Invalid DialogSpec structure: {}", e)),
                )
            })?
        }
    };

    let result = ask_gui(spec).await;

    let json_result = serde_json::to_value(&result).map_err(|e| {
        JsError::from_native(
            JsNativeError::error().with_message(format!("Failed to serialize DialogResult: {}", e)),
        )
    })?;

    let mut ctx_borrow = ctx.borrow_mut();
    JsValue::from_json(&json_result, &mut ctx_borrow).map_err(|e| {
        JsError::from_native(
            JsNativeError::error()
                .with_message(format!("Failed to convert DialogResult to JsValue: {}", e)),
        )
    })
}

async fn ask_gui(spec: DialogSpec) -> DialogResult {
    let (reply_tx, reply_rx) = oneshot::channel();
    let response = Arc::new(RwLock::new(Some(reply_tx)));

    send_message(GuiMessage::ShowCustomDialog(spec, response));

    reply_rx.await.unwrap_or_else(|_| DialogResult {
        fields: Vec::new(),
        buttons: Vec::new(),
        pushed: None,
        values: std::collections::HashMap::new(),
    })
}

fn credentials_spec(title: Option<String>, user: String, error: Option<&str>) -> DialogSpec {
    let field = |name: &str, label: &str, field_type, value: String| DialogField {
        name: name.to_string(),
        label: Some(label.to_string()),
        size: None,
        field_type,
        value,
        choices: None,
    };
    let button = |name: &str, label: &str, primary: bool| DialogButton {
        name: name.to_string(),
        label: label.to_string(),
        close_on_click: true,
        primary,
    };
    DialogSpec {
        title: title.unwrap_or_else(|| "UDS Credentials".to_string()),
        fields: error
            .map(|e| field("error", "", DialogFieldType::Text, e.to_string()))
            .into_iter()
            .chain([
                field("user", "Username", DialogFieldType::String, user),
                field("password", "Password", DialogFieldType::Password, String::new()),
            ])
            .collect(),
        buttons: vec![button("cancel", "Cancel", false), button("connect", "Connect", true)],
        width: None,
        height: None,
    }
}

#[derive(Debug, Default, serde::Deserialize)]
struct CredentialsRequest {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    user: Option<String>,
}

// Resolves to { user, password }, or null when the user cancels or closes the window.
async fn credentials_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &std::cell::RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let request: CredentialsRequest = match args.get(0) {
        Some(arg) if !arg.is_undefined() && !arg.is_null() => {
            let json_val = arg.to_json(&mut *ctx.borrow_mut())?.unwrap_or_default();
            serde_json::from_value(json_val).map_err(|e| {
                JsError::from_native(
                    JsNativeError::typ()
                        .with_message(format!("Invalid credentials request: {}", e)),
                )
            })?
        }
        _ => CredentialsRequest::default(),
    };

    // Empty credentials must never reach the script: it would open the tunnel and RDP.start
    // would ask again while the tunnel wait time runs out.
    let mut user = request.user.unwrap_or_default();
    let mut error = None;
    loop {
        let result = ask_gui(credentials_spec(request.title.clone(), user, error)).await;
        if result.pushed.as_deref() != Some("connect") {
            return Ok(JsValue::null());
        }
        let mut values = result.values;
        user = values.remove("user").unwrap_or_default();
        let password = values.remove("password").unwrap_or_default();
        match CredentialsState::validate(&user, &password) {
            Ok(()) => {
                let credentials = serde_json::json!({ "user": user, "password": password });
                return JsValue::from_json(&credentials, &mut *ctx.borrow_mut());
            }
            Err((_, message)) => error = Some(message),
        }
    }
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "Dialog",
        // Sync functions
        [],
        // Async functions
        [
            ("show", show_dialog_fn, 1),
            ("ask", show_dialog_fn, 1),
            ("credentials", credentials_fn, 1),
        ],
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_context, exec_script_with_result};
    use flume::{Receiver, Sender, bounded};

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_dialog_show() -> Result<()> {
        let (messages_tx, messages_rx): (Sender<GuiMessage>, Receiver<GuiMessage>) = bounded(1);

        crate::gui::set_sender(messages_tx);

        tokio::spawn(async move {
            if let Ok(GuiMessage::ShowCustomDialog(spec, resp)) = messages_rx.recv_async().await {
                assert_eq!(spec.title, "Test Dialog");
                assert_eq!(spec.fields.len(), 2);
                let sender = resp.write().unwrap().take().unwrap();
                let mut values = std::collections::HashMap::new();
                values.insert("user".to_string(), "admin".to_string());
                values.insert("pass".to_string(), "secret123".to_string());
                let result = DialogResult {
                    fields: vec![
                        gui::types::DialogFieldResult {
                            name: "user".to_string(),
                            field_type: gui::types::DialogFieldType::String,
                            value: "admin".to_string(),
                        },
                        gui::types::DialogFieldResult {
                            name: "pass".to_string(),
                            field_type: gui::types::DialogFieldType::Password,
                            value: "secret123".to_string(),
                        },
                    ],
                    buttons: vec![gui::types::DialogButtonResult {
                        name: "ok".to_string(),
                        label: "OK".to_string(),
                        pushed: true,
                    }],
                    pushed: Some("ok".to_string()),
                    values,
                };
                let _ = sender.send(result);
            }
        });

        let mut ctx = create_context(None)?;
        register(&mut ctx)?;

        let script = r#"
            (async () => {
                let res = await Dialog.show({
                    title: "Test Dialog",
                    fields: [
                        { name: "user", type: "string", value: "guest" },
                        { name: "pass", type: "password", value: "" }
                    ],
                    buttons: [
                        { name: "ok", label: "OK" }
                    ]
                });
                return res.pushed + ":" + res.values.user;
            })()
        "#;

        let res = exec_script_with_result(&mut ctx, script).await?;
        let res_str: String = res
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;
        assert_eq!(res_str, "ok:admin");

        Ok(())
    }

    fn reply_with(pushed: Option<&str>, user: &str, password: &str) -> DialogResult {
        let mut values = std::collections::HashMap::new();
        values.insert("user".to_string(), user.to_string());
        values.insert("password".to_string(), password.to_string());
        DialogResult {
            fields: Vec::new(),
            buttons: Vec::new(),
            pushed: pushed.map(str::to_string),
            values,
        }
    }

    async fn run_credentials(script: &str, reply: DialogResult) -> Result<(DialogSpec, String)> {
        let (messages_tx, messages_rx): (Sender<GuiMessage>, Receiver<GuiMessage>) = bounded(1);
        crate::gui::set_sender(messages_tx);

        let gui = tokio::spawn(async move {
            match messages_rx.recv_async().await {
                Ok(GuiMessage::ShowCustomDialog(spec, resp)) => {
                    let sender = resp.write().unwrap().take().unwrap();
                    let _ = sender.send(reply);
                    Some(spec)
                }
                _ => None,
            }
        });

        let mut ctx = create_context(None)?;
        register(&mut ctx)?;
        let res = exec_script_with_result(&mut ctx, script).await?;
        let res_str: String = res
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;
        let spec = gui.await?.ok_or_else(|| anyhow::anyhow!("No dialog was requested"))?;
        Ok((spec, res_str))
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_credentials_connect_returns_user_and_password() -> Result<()> {
        let script = r#"
            (async () => {
                const c = await Dialog.credentials({ user: "DOMAIN\\alice" });
                return c.user + "|" + c.password;
            })()
        "#;
        let (spec, res) =
            run_credentials(script, reply_with(Some("connect"), "DOMAIN\\bob", "s3cret")).await?;

        assert_eq!(res, "DOMAIN\\bob|s3cret");
        assert_eq!(spec.title, "UDS Credentials");
        let names: Vec<_> = spec.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["user", "password"]);
        assert_eq!(spec.fields[0].value, "DOMAIN\\alice");
        assert!(matches!(spec.fields[1].field_type, DialogFieldType::Password));
        assert!(spec.fields[1].value.is_empty());
        assert!(spec.buttons.iter().any(|b| b.name == "connect" && b.primary));
        Ok(())
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_credentials_cancel_returns_null() -> Result<()> {
        let script = r#"
            (async () => {
                const c = await Dialog.credentials();
                return c === null ? "null" : "not-null";
            })()
        "#;
        let (spec, res) = run_credentials(script, reply_with(Some("cancel"), "x", "y")).await?;
        assert_eq!(res, "null");
        assert!(spec.fields[0].value.is_empty());
        Ok(())
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_credentials_window_closed_returns_null() -> Result<()> {
        let script = r#"
            (async () => {
                const c = await Dialog.credentials({ title: "Remote desktop" });
                return c === null ? "null" : "not-null";
            })()
        "#;
        let (spec, res) = run_credentials(script, reply_with(None, "x", "y")).await?;
        assert_eq!(res, "null");
        assert_eq!(spec.title, "Remote desktop");
        Ok(())
    }


    // Same flow as the RDPEmbedded transport scripts on the broker: ask only when credentials are missing.
    const ASK_THEN_START: &str = r#"
        import { Dialog, RDP } from 'runtime';

        if (!data.user || !data.password) {
            const credentials = await Dialog.credentials({ user: data.user });
            if (credentials === null) {
                throw new Error('Connection cancelled by user');
            }
            data.user = credentials.user;
            data.password = credentials.password;
        }

        RDP.start(data);
    "#;

    async fn run_transport_script(
        data: serde_json::Value,
        reply: Option<DialogResult>,
    ) -> (Result<()>, Vec<GuiMessage>) {
        let (messages_tx, messages_rx): (Sender<GuiMessage>, Receiver<GuiMessage>) = bounded(4);
        crate::gui::set_sender(messages_tx);

        let gui = tokio::spawn(async move {
            let mut seen = Vec::new();
            while let Ok(msg) = messages_rx.recv_async().await {
                if let GuiMessage::ShowCustomDialog(_, resp) = &msg {
                    let sender = resp.write().unwrap().take().unwrap();
                    let _ = sender.send(reply.clone().expect("dialog was not expected"));
                }
                let done = matches!(msg, GuiMessage::ConnectRdp(_));
                seen.push(msg);
                if done {
                    break;
                }
            }
            seen
        });

        let result = crate::run_js(ASK_THEN_START, Some(data)).await;
        crate::gui::set_sender(bounded(1).0);
        let seen = tokio::time::timeout(std::time::Duration::from_secs(2), gui)
            .await
            .map(|r| r.unwrap_or_default())
            .unwrap_or_default();
        (result, seen)
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_script_with_credentials_does_not_ask() {
        let data = serde_json::json!({ "server": "10.0.0.1", "user": "alice", "password": "pw" });
        let (result, seen) = run_transport_script(data, None).await;
        assert!(result.is_ok(), "{result:?}");
        assert!(!seen.iter().any(|m| matches!(m, GuiMessage::ShowCustomDialog(..))));
        match seen.last() {
            Some(GuiMessage::ConnectRdp(s)) => assert_eq!((s.user.as_str(), s.password.as_str()), ("alice", "pw")),
            other => panic!("Expected ConnectRdp, got {other:?}"),
        }
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_script_without_password_asks_and_connects() {
        let data = serde_json::json!({ "server": "10.0.0.1", "user": "alice", "password": "" });
        let reply = reply_with(Some("connect"), "alice", "typed");
        let (result, seen) = run_transport_script(data, Some(reply)).await;
        assert!(result.is_ok(), "{result:?}");
        assert!(matches!(seen.first(), Some(GuiMessage::ShowCustomDialog(..))));
        match seen.last() {
            Some(GuiMessage::ConnectRdp(s)) => assert_eq!(s.password, "typed"),
            other => panic!("Expected ConnectRdp, got {other:?}"),
        }
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_script_cancelled_does_not_connect() {
        let data = serde_json::json!({ "server": "10.0.0.1", "user": "", "password": "" });
        let (result, seen) = run_transport_script(data, Some(reply_with(Some("cancel"), "", ""))).await;
        assert!(result.is_err());
        assert!(!seen.iter().any(|m| matches!(m, GuiMessage::ConnectRdp(_))));
    }


    async fn run_credentials_replies(
        script: &str,
        replies: Vec<DialogResult>,
    ) -> Result<(Vec<DialogSpec>, String)> {
        let (messages_tx, messages_rx): (Sender<GuiMessage>, Receiver<GuiMessage>) = bounded(1);
        crate::gui::set_sender(messages_tx);

        let gui = tokio::spawn(async move {
            let mut specs = Vec::new();
            for reply in replies {
                match messages_rx.recv_async().await {
                    Ok(GuiMessage::ShowCustomDialog(spec, resp)) => {
                        let sender = resp.write().unwrap().take().unwrap();
                        let _ = sender.send(reply);
                        specs.push(spec);
                    }
                    _ => break,
                }
            }
            specs
        });

        let mut ctx = create_context(None)?;
        register(&mut ctx)?;
        let res = tokio::time::timeout(TEST_TIMEOUT, exec_script_with_result(&mut ctx, script))
            .await
            .map_err(|_| anyhow::anyhow!("Script did not finish"))??;
        let res_str: String = res
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;
        crate::gui::set_sender(bounded(1).0);
        let specs = tokio::time::timeout(TEST_TIMEOUT, gui).await??;
        Ok((specs, res_str))
    }

    const TEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

    const ASK_CREDENTIALS: &str = r#"
        (async () => {
            const c = await Dialog.credentials({ user: "alice" });
            return c === null ? "null" : c.user + "|" + c.password;
        })()
    "#;

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_credentials_empty_password_asks_again_with_error() -> Result<()> {
        let replies = vec![
            reply_with(Some("connect"), "alice", ""),
            reply_with(Some("connect"), "alice", "s3cret"),
        ];
        let (specs, res) = run_credentials_replies(ASK_CREDENTIALS, replies).await?;

        assert_eq!(res, "alice|s3cret");
        assert_eq!(specs.len(), 2);
        assert!(!specs[0].fields.iter().any(|f| f.name == "error"));
        let error = specs[1].fields.iter().find(|f| f.name == "error").expect("error field");
        assert_eq!(error.value, "Password is required");
        let user = specs[1].fields.iter().find(|f| f.name == "user").unwrap();
        assert_eq!(user.value, "alice", "typed user is kept on retry");
        Ok(())
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_credentials_blank_user_asks_again_with_error() -> Result<()> {
        let replies = vec![
            reply_with(Some("connect"), "   ", "pw"),
            reply_with(Some("connect"), "bob", "pw"),
        ];
        let (specs, res) = run_credentials_replies(ASK_CREDENTIALS, replies).await?;

        assert_eq!(res, "bob|pw");
        let error = specs[1].fields.iter().find(|f| f.name == "error").expect("error field");
        assert_eq!(error.value, "Username is required");
        Ok(())
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_credentials_cancel_after_validation_error_returns_null() -> Result<()> {
        let replies = vec![
            reply_with(Some("connect"), "", ""),
            reply_with(Some("cancel"), "", ""),
        ];
        let (specs, res) = run_credentials_replies(ASK_CREDENTIALS, replies).await?;

        assert_eq!(res, "null");
        assert_eq!(specs.len(), 2);
        Ok(())
    }

    #[tokio::test]
    #[serial_test::serial(js_modules)]
    async fn test_script_empty_submit_does_not_open_tunnel_or_rdp() {
        let data = serde_json::json!({ "server": "10.0.0.1", "user": "", "password": "" });
        let (messages_tx, messages_rx): (Sender<GuiMessage>, Receiver<GuiMessage>) = bounded(4);
        crate::gui::set_sender(messages_tx);

        let gui = tokio::spawn(async move {
            let mut dialogs = 0;
            let mut rdp_after_valid = None;
            while let Ok(msg) = messages_rx.recv_async().await {
                match msg {
                    GuiMessage::ShowCustomDialog(_, resp) => {
                        dialogs += 1;
                        let reply = if dialogs == 1 {
                            reply_with(Some("connect"), "", "")
                        } else {
                            reply_with(Some("connect"), "carol", "pw")
                        };
                        let sender = resp.write().unwrap().take().unwrap();
                        let _ = sender.send(reply);
                    }
                    GuiMessage::ConnectRdp(s) => {
                        rdp_after_valid = Some((dialogs, s.user.clone(), s.password.clone()));
                        break;
                    }
                    _ => {}
                }
            }
            rdp_after_valid
        });

        let result = tokio::time::timeout(TEST_TIMEOUT, crate::run_js(ASK_THEN_START, Some(data)))
            .await
            .expect("script did not finish");
        crate::gui::set_sender(bounded(1).0);
        assert!(result.is_ok(), "{result:?}");
        let rdp = tokio::time::timeout(TEST_TIMEOUT, gui)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(rdp, Some((2, "carol".to_string(), "pw".to_string())));
    }

}
