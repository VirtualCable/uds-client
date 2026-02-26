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
use anyhow::Result;
use boa_engine::{
    Context, JsResult, JsValue,
    error::{JsError, JsNativeError},
    js_string,
    object::ObjectInitializer,
    property::Attribute,
    value::TryFromJs,
};

use connection::{tasks, types::TunnelConnectInfo};
use shared::log;

fn add_early_unlinkable_file_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let file_path = extract_js_args!(args, ctx, String);

    tasks::add_early_unlinkable_file(file_path);

    Ok(JsValue::undefined())
}

fn add_late_unlinkable_file_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let file_path = extract_js_args!(args, ctx, String);

    tasks::add_late_unlinkable_file(file_path);

    Ok(JsValue::undefined())
}

fn add_waitable_app_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let task_handle = extract_js_args!(args, ctx, u32);

    tasks::add_waitable_app(task_handle);

    Ok(JsValue::undefined())
}

// Struct for tunnel start parameters
#[derive(TryFromJs, Default)]
struct TunnelParams {
    addr: String,
    port: u16,
    ticket: String,
    startup_time_ms: Option<u64>,
    check_certificate: Option<bool>,
    local_port: Option<u16>,
    keep_listening_after_timeout: Option<bool>,
    enable_ipv6: Option<bool>,
    shared_secret: Option<Vec<u8>>,
}

async fn start_tunel_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &std::cell::RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let tunnel_info = {
        let mut ctx_borrow = ctx.borrow_mut();
        let params = extract_js_args!(args, &mut *ctx_borrow, TunnelParams);
        log::debug!(
            "Starting tunnel to {}:{} with ticket {}, check_certificate: {:?}, listen_timeout_ms: {:?}, local_port: {:?}, keep_listening_after_timeout: {:?}, enable_ipv6: {:?}, shared_secret: {:?}",
            params.addr,
            params.port,
            params.ticket,
            params.check_certificate,
            params.startup_time_ms,
            params.local_port,
            params.keep_listening_after_timeout,
            params.enable_ipv6,
            params.shared_secret,
        );
        TunnelConnectInfo {
            addr: params.addr,
            port: params.port,
            ticket: params.ticket.as_bytes().try_into().map_err(|_| {
                JsError::from_native(
                    JsNativeError::error()
                        .with_message("Invalid ticket length, must be 32 bytes".to_string()),
                )
            })?,
            check_certificate: params.check_certificate.unwrap_or(true),
            local_port: params.local_port,
            startup_time_ms: params.startup_time_ms.unwrap_or(0),
            keep_listening_after_timeout: params.keep_listening_after_timeout.unwrap_or(false),
            enable_ipv6: params.enable_ipv6.unwrap_or(false),
            shared_secret: params.shared_secret.as_ref().map(|s| {
                s.as_slice()
                    .try_into()
                    .map_err(|_| {
                        JsError::from_native(
                            JsNativeError::error().with_message(
                                "Invalid shared secret length".to_string(),
                            ),
                        )
                    })
            }).transpose()?,
        }
    };

    let port = connection::start_tunnel(tunnel_info)
        .await
        .map(JsValue::from)
        .map_err(|e| JsError::from_native(JsNativeError::error().with_message(format!("{}", e))))?;

    // Re-borrow the context to create the result object
    let result = {
        let mut ctx_borrow = ctx.borrow_mut();
        ObjectInitializer::new(*ctx_borrow)
            .property(js_string!("port"), port, Attribute::READONLY)
            .build()
    };

    // Note: comments for future reference, not a real case
    // let error_function = FunctionObjectBuilder::new(
    //         ctx.realm(),
    //         NativeFunction::from_fn_ptr(error_fn)
    //     )
    //     .name(js_string!("error"))
    //     .length(1)
    //     .build();

    Ok(JsValue::from(result))
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "Tasks",
        // Sync functions
        [
            ("addEarlyUnlinkableFile", add_early_unlinkable_file_fn, 1),
            ("addLateUnlinkableFile", add_late_unlinkable_file_fn, 1),
            ("addWaitableApp", add_waitable_app_fn, 1),
        ],
        // Async functions
        [("startTunnel", start_tunel_fn, 8),],
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::log;
    use crate::{create_context, exec_script};

    use super::*;

    use anyhow::Result;

    #[tokio::test]
    async fn test_add_early_unlinkable_file() -> Result<()> {
        log::setup_logging("debug", log::LogType::Test);
        let mut ctx = create_context(None)?;
        register(&mut ctx)?;

        let script = r#"
            Tasks.addEarlyUnlinkableFile("file_to_delete_early.txt");
        "#;
        _ = exec_script(&mut ctx, script).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_add_late_unlinkable_file() -> Result<()> {
        log::setup_logging("debug", log::LogType::Test);
        let mut ctx = create_context(None)?;
        register(&mut ctx)?;

        let script = r#"
            Tasks.addLateUnlinkableFile("file_to_delete_late.txt");
        "#;
        _ = exec_script(&mut ctx, script).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_add_waitable_app() -> Result<()> {
        log::setup_logging("debug", log::LogType::Test);
        let mut ctx = create_context(None)?;
        register(&mut ctx)?;
        let script = r#"
            Tasks.addWaitableApp(12345);
        "#;
        _ = exec_script(&mut ctx, script).await;
        Ok(())
    }
}
