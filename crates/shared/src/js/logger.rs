use anyhow::Result;
use boa_engine::{Context, JsResult, JsValue};

use crate::log;

// log(level: String, msg: String)
fn trace_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let msg = extract_js_args!(args, ctx, String);

    log::trace!("{}", msg);
    Ok(JsValue::undefined())
}

fn debug_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let msg = extract_js_args!(args, ctx, String);

    log::debug!("{}", msg);
    Ok(JsValue::undefined())
}

fn info_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let msg = extract_js_args!(args, ctx, String);

    log::info!("{}", msg);
    Ok(JsValue::undefined())
}

fn warn_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let msg = extract_js_args!(args, ctx, String);

    log::warn!("{}", msg);
    Ok(JsValue::undefined())
}

fn error_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let msg = extract_js_args!(args, ctx, String);

    log::error!("{}", msg);
    Ok(JsValue::undefined())
}

pub fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "Logger",
        [
            ("debug", debug_fn, 1),
            ("trace", trace_fn, 1),
            ("info", info_fn, 1),
            ("warn", warn_fn, 1),
            ("error", error_fn, 1),
        ]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::exec_script;
    use super::*;

    use anyhow::Result;
    use boa_engine::Context;

    #[test]
    fn test_log() -> Result<()> {
        log::setup_logging("trace", log::LogType::Tests);
        let mut ctx = Context::default();

        // Register the utils module
        register(&mut ctx)?;

        // Run a test script
        exec_script(
            &mut ctx,
            r#"
            Logger.trace("Trace message");
            Logger.debug("Debug message");
            Logger.info("Info message");
            Logger.warn("Warn message");
            Logger.error("Error message");
        "#,
        )
        .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        Ok(())
    }
}
