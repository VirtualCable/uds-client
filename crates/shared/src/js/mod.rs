use anyhow::Result;

use boa_engine::{Context, JsResult, JsValue, Source};

// Helpers functions for javascript rust bindings
#[macro_use]
mod macros;

mod helpers;

// Windows specific functions for data protection and registry access
#[cfg(target_os = "windows")]
mod windows;

// Js modules
mod utils;
mod process;


pub fn exec_script(ctx: &mut Context, script: &str) -> JsResult<JsValue> {
    // runtime de un solo hilo
    ctx.eval(Source::from_bytes(script))
}

pub fn run_js(script: &str) -> Result<()> {
    let mut ctx = Context::default();
    utils::register(&mut ctx)?;

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
mod tests;
