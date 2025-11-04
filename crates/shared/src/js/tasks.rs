use anyhow::Result;
use boa_engine::{
    Context, JsResult, JsValue,
    error::{JsError, JsNativeError},
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
        bool,
        u64,
        bool,
        bool
    );

    let tunnel_info = tunnel::TunnelConnectInfo {
        addr,
        port,
        ticket,
        local_port,
        check_certificate,
        listen_timeout_ms,
        keep_listening_after_timeout,
        enable_ipv6,
    };

    tunnel::start_tunnel(tunnel_info)
        .map(JsValue::from)
        .map_err(|e| JsError::from_native(JsNativeError::error().with_message(format!("{}", e))))?;

    Ok(JsValue::undefined())
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "Tasks",
        [
            ("add_early_unlinkable_file", add_early_unlinkable_file_fn, 1),
            ("add_late_unlinkable_file", add_late_unlinkable_file_fn, 1),
            ("add_waitable_app", add_waitable_app_fn, 1),
            ("start_tunnel", start_tunel_fn, 8),
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
            Tasks.add_early_unlinkable_file("file_to_delete_early.txt");
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
            Tasks.add_late_unlinkable_file("file_to_delete_late.txt");
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
            Tasks.add_waitable_app(12345);
        "#;
        _ = exec_script(&mut ctx, script);
        Ok(())
    }
}
