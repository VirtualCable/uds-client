use anyhow::Result;

use boa_engine::{Context, JsResult, JsValue, Source, js_string};

use crate::log;

// Helpers functions for javascript rust bindings
#[macro_use]
mod macros;

mod helpers;

// Js modules
mod file;
mod logger;
mod process;
mod tasks;
mod utils;

pub fn init_ctx(ctx: &mut Context) -> Result<()> {
    utils::register(ctx)?;
    logger::register(ctx)?;
    process::register(ctx)?;
    tasks::register(ctx)?;
    file::register(ctx)?;
    Ok(())
}

pub fn exec_script(ctx: &mut Context, script: &str) -> JsResult<JsValue> {
    // runtime de un solo hilo
    ctx.eval(Source::from_bytes(script))
}

pub fn run_js(script: &str, data: Option<serde_json::Value>) -> Result<()> {
    log::debug!("Running JS script:\n");
    let mut ctx = Context::default();
    init_ctx(&mut ctx)?;
    if let Some(data) = data {
        let js_value = JsValue::from_json(&data, &mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert JSON data to JsValue: {}", e))?;

        ctx.register_global_property(
            js_string!("data"),
            js_value,
            boa_engine::property::Attribute::empty(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to register global property: {}", e))?;
    } else {
        ctx.register_global_property(
            js_string!("data"),
            JsValue::undefined(),
            boa_engine::property::Attribute::empty(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to register global property: {}", e))?;
    }

    let res = exec_script(&mut ctx, script);
    if res.is_err() {
        Err(anyhow::anyhow!(
            "JavaScript execution error: {}",
            res.err().unwrap()
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log;
    use anyhow::Result;
    use boa_engine::Context;

    #[test]
    fn test_init_ctx() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = Context::default();
        init_ctx(&mut ctx)?;
        // Run a simple script to verify that modules are registered
        let script = r#"
            let tempDir = File.getTempDirectory();
            let homeDir = File.getHomeDirectory();
            tempDir + " | " + homeDir;
        "#;
        let result = exec_script(&mut ctx, script)
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        let result: String = result
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;

        log::info!("Script result: {}", result);
        let home_directory = if cfg!(target_os = "windows") {
            std::env::var("USERPROFILE").unwrap_or_default()
        } else {
            std::env::var("HOME").unwrap_or_default()
        };
        assert!(result.contains(&home_directory));

        assert!(result.contains(std::env::temp_dir().to_string_lossy().as_ref()));

        Ok(())
    }

    #[test]
    fn test_exec_script() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = Context::default();
        let script = r#"
            let a = 5;
            let b = 10;
            a + b;
        "#;
        let result = exec_script(&mut ctx, script)
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;
        let result: i32 = result
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;
        assert_eq!(result, 15);
        Ok(())
    }

    #[test]
    fn test_run_js_with_data() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let script = r#"
            let result = data.value1 + data.value2;
            result;
        "#;
        let data = serde_json::json!({
            "value1": 20,
            "value2": 22
        });
        let mut ctx = Context::default();
        init_ctx(&mut ctx)?;
        run_js(script, Some(data))?;
        Ok(())
    }
}
