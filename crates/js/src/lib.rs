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
use std::rc::Rc;

use anyhow::Result;
use boa_engine::{
    Context, JsValue, Module, js_string,
    module::{MapModuleLoader, SyntheticModuleInitializer},
};

use shared::{
    broker::api::types::{Script, ScriptType},
    log,
};

pub mod gui;

// Helpers functions for javascript rust bindings
#[macro_use]
mod macros;

mod executor;
mod helpers;

mod js_modules;

pub use executor::{create_context, exec_script, exec_script_with_result};

fn init_runtime(ctx: &mut Context) -> Result<()> {
    js_modules::register(ctx)?;
    Ok(())
}

fn create_runtime_module(ctx: &mut Context) -> Module {
    let global = ctx.global_object();
    let process = global.get(js_string!("Process"), ctx).unwrap();
    let logger = global.get(js_string!("Logger"), ctx).unwrap();
    let file = global.get(js_string!("File"), ctx).unwrap();
    let utils = global.get(js_string!("Utils"), ctx).unwrap();
    let tasks = global.get(js_string!("Tasks"), ctx).unwrap();
    let rdp = global.get(js_string!("RDP"), ctx).unwrap();

    Module::synthetic(
        &[
            js_string!("Process"),
            js_string!("Logger"),
            js_string!("File"),
            js_string!("Utils"),
            js_string!("Tasks"),
            js_string!("RDP"),
        ],
        SyntheticModuleInitializer::from_copy_closure_with_captures(
            move |module: &boa_engine::module::SyntheticModule,
                  (process, logger, file, utils, tasks, rdp),
                  _ctx| {
                module.set_export(&js_string!("Process"), process.clone())?;
                module.set_export(&js_string!("Logger"), logger.clone())?;
                module.set_export(&js_string!("File"), file.clone())?;
                module.set_export(&js_string!("Utils"), utils.clone())?;
                module.set_export(&js_string!("Tasks"), tasks.clone())?;
                module.set_export(&js_string!("RDP"), rdp.clone())?;
                Ok(())
            },
            (process, logger, file, utils, tasks, rdp),
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
        for frame in ctx.stack_trace() {
            log::error!(
                "  at {:?} (line: {})",
                frame.position().position,
                frame.position().path,
                // The line information is available in the frame
            );
        }
        let error = res.err().unwrap();
        log::error!("JavaScript execution error: {}", error);
        Err(anyhow::anyhow!("JavaScript execution error: {}", error))
    } else {
        Ok(())
    }
}

pub async fn run_script(script: &Script) -> Result<()> {
    // If not javascript type, return error
    if script.script_type != ScriptType::Javascript {
        return Err(anyhow::anyhow!(
            "Unsupported script type: {}",
            script.script_type
        ));
    }
    let script_content = script.decoded_script()?;
    let params = script.decoded_params()?;

    run_js(&script_content, Some(params)).await
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
