use anyhow::Result;

use boa_engine::{
    Context, JsResult, JsString, JsValue, Source, js_string,
    native_function::NativeFunction,
};
use std::{cell::RefCell, fs, process::Command};

/// log(msg)
fn log_fn(_: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    let msg = args
        .first()
        .and_then(JsValue::as_string)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    println!("📝 {}", msg);
    Ok(JsValue::undefined())
}

/// create_file(path)
fn create_file_fn(_: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    let path = args
        .first()
        .and_then(JsValue::as_string)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    fs::File::create(path).unwrap();
    Ok(JsValue::undefined())
}

/// write_file(path, content)
fn write_file_fn(_: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    let path = args
        .first()
        .and_then(JsValue::as_string)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let content = args
        .get(1)
        .and_then(JsValue::as_string)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    fs::write(path, content).unwrap();
    Ok(JsValue::undefined())
}

/// find(path, pattern)
fn find_fn(_: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    let path = args
        .first()
        .and_then(JsValue::as_string)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let pattern = args
        .get(1)
        .and_then(JsValue::as_string)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let matches: Vec<String> = fs::read_dir(path)
        .unwrap()
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().to_string_lossy().to_string();
            if name.contains(&pattern) {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    Ok(JsValue::from(JsString::from(matches.join(", "))))
}

/// run_async(cmd) → Promise<string>
async fn run_async_fn(
    _: &JsValue,
    args: &[JsValue],
    _ctx: &RefCell<&mut Context>,
) -> JsResult<JsValue> {
    println!("🚀 Running async command...");
    let cmd = args
        .first()
        .and_then(JsValue::as_string)
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let out = tokio::task::spawn_blocking(move || Command::new(cmd).output())
        .await
        .unwrap()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    Ok(JsValue::from(JsString::from(stdout)))
}

/// Registra todas las funciones globales
fn register_io_bindings(ctx: &mut Context) -> JsResult<()> {
    ctx.register_global_callable(js_string!("log"), 1, NativeFunction::from_fn_ptr(log_fn))?;
    ctx.register_global_callable(
        js_string!("create_file"),
        1,
        NativeFunction::from_fn_ptr(create_file_fn),
    )?;
    ctx.register_global_callable(
        js_string!("write_file"),
        2,
        NativeFunction::from_fn_ptr(write_file_fn),
    )?;
    ctx.register_global_callable(js_string!("find"), 2, NativeFunction::from_fn_ptr(find_fn))?;
    ctx.register_global_callable(
        js_string!("run_async"),
        1,
        NativeFunction::from_async_fn(run_async_fn),
    )?;
    Ok(())
}

pub fn run_js(script: &str) -> Result<()> {
    let mut ctx = Context::default();
    register_io_bindings(&mut ctx)
        .map_err(|e| anyhow::anyhow!("Failed to register IO bindings: {}", e))?;

    // Initialize tokio runtime, an execute the script inside it to support async functions
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build Tokio runtime: {}", e))?;

    let mut err = JsResult::Ok(JsValue::undefined());
    let err_ref = &mut err;
    rt.block_on(async move {
        *err_ref = ctx.eval(Source::from_bytes(script));
        if err_ref.is_err() {
            return;
        }
        _ = ctx.run_jobs();
    });
    if let Err(e) = err {
        return Err(anyhow::anyhow!("JavaScript error: {}", e));
    }
    Ok(())
}
