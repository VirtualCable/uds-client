use anyhow::Result;
use boa_engine::{
    Context, JsResult, JsValue,
    error::{JsError, JsNativeError},
    js_string,
    object::ObjectInitializer,
    property::Attribute,
};

use crate::{tasks, tunnel};

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

async fn start_tunel_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &std::cell::RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let tunnel_info = {
        let mut ctx_borrow = ctx.borrow_mut();
        let (
            addr,
            port,
            ticket,
            listen_timeout_ms,
            local_port,
            check_certificate,
            keep_listening_after_timeout,
            enable_ipv6,
        ) = extract_js_args!(
            args,
            &mut *ctx_borrow,
            String,
            u16,
            String,
            Option<u64>,
            Option<u16>,
            Option<bool>,
            Option<bool>,
            Option<bool>
        );
        tunnel::TunnelConnectInfo {
            addr,
            port,
            ticket,
            local_port,
            check_certificate: check_certificate.unwrap_or(true),
            listen_timeout_ms: listen_timeout_ms.unwrap_or(0),
            keep_listening_after_timeout: keep_listening_after_timeout.unwrap_or(false),
            enable_ipv6: enable_ipv6.unwrap_or(false),
        }
    };

    let port = tunnel::start_tunnel(tunnel_info)
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
    use super::super::{exec_script, create_context};
    use crate::log;

    use super::*;

    use anyhow::Result;

    #[tokio::test]
    async fn test_add_early_unlinkable_file() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
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
        log::setup_logging("debug", log::LogType::Tests);
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
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;
        register(&mut ctx)?;
        let script = r#"
            Tasks.addWaitableApp(12345);
        "#;
        _ = exec_script(&mut ctx, script).await;
        Ok(())
    }
}
