use std::rc::Rc;

use anyhow::Result;

use boa_engine::{Context, JsValue, Module, js_string, module::{SyntheticModuleInitializer, MapModuleLoader}};

use crate::log;

// Helpers functions for javascript rust bindings
#[macro_use]
mod macros;

mod executor;
mod helpers;

// Js modules
mod file;
mod logger;
mod process;
mod tasks;
mod utils;

pub use executor::{create_context, exec_script, exec_script_with_result};

fn init_runtime(ctx: &mut Context) -> Result<()> {
    utils::register(ctx)?;
    logger::register(ctx)?;
    process::register(ctx)?;
    tasks::register(ctx)?;
    file::register(ctx)?;
    Ok(())
}

fn create_runtime_module(ctx: &mut Context) -> Module {
    let global = ctx.global_object();
    let process = global.get(js_string!("Process"), ctx).unwrap();
    let logger = global.get(js_string!("Logger"), ctx).unwrap();
    let file = global.get(js_string!("File"), ctx).unwrap();
    let utils = global.get(js_string!("Utils"), ctx).unwrap();
    let tasks = global.get(js_string!("Tasks"), ctx).unwrap();

    Module::synthetic(
        &[
            js_string!("Process"),
            js_string!("Logger"),
            js_string!("File"),
            js_string!("Utils"),
            js_string!("Tasks"),
        ],
        SyntheticModuleInitializer::from_copy_closure_with_captures(
            move |module: &boa_engine::module::SyntheticModule,
                  (process, logger, file, utils, tasks),
                  _ctx| {
                module.set_export(&js_string!("Process"), process.clone())?;
                module.set_export(&js_string!("Logger"), logger.clone())?;
                module.set_export(&js_string!("File"), file.clone())?;
                module.set_export(&js_string!("Utils"), utils.clone())?;
                module.set_export(&js_string!("Tasks"), tasks.clone())?;
                Ok(())
            },
            (process, logger, file, utils, tasks),
        ),
        None,
        None,
        ctx,
    )
}

pub async fn run_js(script: &str, data: Option<serde_json::Value>) -> Result<()> {
    log::debug!("Running JS script:\n");

    let loader = Rc::new(MapModuleLoader::new());

    let mut ctx = create_context(Some(loader.clone()))?;
    init_runtime(&mut ctx)?;

    let runtime_module = create_runtime_module(&mut ctx);
    loader.insert("runtime", runtime_module);

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

    let res = exec_script(&mut ctx, script).await;
    if res.is_err() {
        let error = res.err().unwrap();
        log::error!("JavaScript execution error: {}", error);
        Err(anyhow::anyhow!("JavaScript execution error: {}", error))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log;
    use anyhow::Result;

    #[tokio::test]
    async fn test_init_ctx() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;
        init_runtime(&mut ctx)?;

        // Run a simple script to verify that modules are registered
        let script = r#"
            let tempDir = File.getTempDirectory();
            let homeDir = File.getHomeDirectory();
            tempDir + " | " + homeDir;
        "#;
        let result = exec_script_with_result(&mut ctx, script)
            .await
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

    #[tokio::test]
    async fn test_exec_script() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;
        let script = r#"
            let a = 5;
            let b = 10;
            a + b;
        "#;
        let result = exec_script_with_result(&mut ctx, script)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;
        let result: i32 = result
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;
        assert_eq!(result, 15);
        Ok(())
    }

    #[tokio::test]
    async fn test_run_js_with_data() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let script = r#"
            let result = data.value1 + data.value2;
            result;
        "#;
        let data = serde_json::json!({
            "value1": 20,
            "value2": 22
        });
        run_js(script, Some(data)).await?;
        Ok(())
    }
}
