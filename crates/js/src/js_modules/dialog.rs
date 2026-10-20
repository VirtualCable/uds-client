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

use gui::types::{DialogResult, DialogSpec, GuiMessage};

use crate::gui::send_message;

async fn show_dialog_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &std::cell::RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let spec: DialogSpec = {
        let mut ctx_borrow = ctx.borrow_mut();
        let arg = args.get(0).ok_or_else(|| {
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
                .to_json(&mut *ctx_borrow)
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

    let (reply_tx, reply_rx) = oneshot::channel();
    let response = Arc::new(RwLock::new(Some(reply_tx)));

    send_message(GuiMessage::ShowCustomDialog(spec, response));

    let result = reply_rx.await.unwrap_or_else(|_| DialogResult {
        fields: Vec::new(),
        buttons: Vec::new(),
        pushed: None,
        values: std::collections::HashMap::new(),
    });

    let json_result = serde_json::to_value(&result).map_err(|e| {
        JsError::from_native(
            JsNativeError::error().with_message(format!("Failed to serialize DialogResult: {}", e)),
        )
    })?;

    let mut ctx_borrow = ctx.borrow_mut();
    JsValue::from_json(&json_result, &mut *ctx_borrow).map_err(|e| {
        JsError::from_native(
            JsNativeError::error()
                .with_message(format!("Failed to convert DialogResult to JsValue: {}", e)),
        )
    })
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "Dialog",
        // Sync functions
        [],
        // Async functions
        [("show", show_dialog_fn, 1), ("ask", show_dialog_fn, 1)],
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
}
