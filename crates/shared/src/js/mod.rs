use anyhow::Result;

use boa_engine::{Context, JsResult, JsValue, Source};

// Helpers functions for javascript rust bindings
#[macro_use]
mod macros;

mod helpers;

// Js modules
mod utils;
mod process;

pub fn init_ctx(ctx: &mut Context) -> Result<()> {
    utils::register(ctx)?;
    process::register(ctx)?;
    Ok(())
}

pub fn exec_script(ctx: &mut Context, script: &str) -> JsResult<JsValue> {
    // runtime de un solo hilo
    ctx.eval(Source::from_bytes(script))
}

pub fn run_js(script: &str) -> Result<()> {
    let mut ctx = Context::default();
    init_ctx(&mut ctx)?;

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
