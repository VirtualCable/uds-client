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

fn start_tunel_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (
        addr,
        port,
        ticket,
        local_port,
        check_certificate,
        listen_timeout_ms,
        keep_listening_after_timeout,
        enable_ipv6,
    ) = extract_js_args!(
        args,
        ctx,
        String,
        u16,
        String,
        Option<u16>,
        Option<bool>,
        Option<u64>,
        Option<bool>,
        Option<bool>
    );

    let tunnel_info = tunnel::TunnelConnectInfo {
        addr,
        port,
        ticket,
        local_port,
        check_certificate: check_certificate.unwrap_or(true),
        listen_timeout_ms: listen_timeout_ms.unwrap_or(0),
        keep_listening_after_timeout: keep_listening_after_timeout.unwrap_or(false),
        enable_ipv6: enable_ipv6.unwrap_or(false),
    };

    let port = tunnel::start_tunnel(tunnel_info)
        .map(JsValue::from)
        .map_err(|e| JsError::from_native(JsNativeError::error().with_message(format!("{}", e))))?;

    // Note: comments for future reference, not a real case
    // let error_function = FunctionObjectBuilder::new(
    //         ctx.realm(),
    //         NativeFunction::from_fn_ptr(error_fn)
    //     )
    //     .name(js_string!("error"))
    //     .length(1)
    //     .build();

    let result = ObjectInitializer::new(ctx)
        .property(js_string!("port"), port, Attribute::READONLY)
        // .property(js_string!("id"), JsValue::from(id), Attribute::READONLY)
        // .property(
        //     js_string!("error"),
        //     error_function,
        //     Attribute::READONLY | Attribute::NON_ENUMERABLE,
        // )
        .build();

    Ok(result.into())
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "Tasks",
        [
            ("addEarlyUnlinkableFile", add_early_unlinkable_file_fn, 1),
            ("addLateUnlinkableFile", add_late_unlinkable_file_fn, 1),
            ("addWaitableApp", add_waitable_app_fn, 1),
            ("startTunnel", start_tunel_fn, 8),
        ]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::exec_script;
    use crate::log;

    use super::*;

    use anyhow::Result;

    #[test]
    fn test_add_early_unlinkable_file() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = Context::default();
        register(&mut ctx)?;

        let script = r#"
            Tasks.addEarlyUnlinkableFile("file_to_delete_early.txt");
        "#;
        _ = exec_script(&mut ctx, script);
        Ok(())
    }

    #[test]
    fn test_add_late_unlinkable_file() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = Context::default();
        register(&mut ctx)?;

        let script = r#"
            Tasks.addLateUnlinkableFile("file_to_delete_late.txt");
        "#;
        _ = exec_script(&mut ctx, script);
        Ok(())
    }

    #[test]
    fn test_add_waitable_app() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = Context::default();
        register(&mut ctx)?;
        let script = r#"
            Tasks.addWaitableApp(12345);
        "#;
        _ = exec_script(&mut ctx, script);
        Ok(())
    }
}
