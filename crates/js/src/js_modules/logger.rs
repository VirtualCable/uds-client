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
use boa_engine::{Context, JsResult, JsValue};

use shared::log;

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
        // Sync functions
        [
            ("debug", debug_fn, 1),
            ("trace", trace_fn, 1),
            ("info", info_fn, 1),
            ("warn", warn_fn, 1),
            ("error", error_fn, 1),
        ],
        // Async functions, none here
        [],
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{create_context, exec_script};

    use super::*;

    use anyhow::Result;

    #[tokio::test]
    async fn test_log() -> Result<()> {
        log::setup_logging("trace", log::LogType::Tests);
        let mut ctx = create_context(None)?;

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
        ).await
        .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        Ok(())
    }
}
