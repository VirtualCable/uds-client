use anyhow::Result;
use boa_engine::{
    Context, JsResult, JsString, JsValue,
    error::{JsError, JsNativeError},
};

use is_executable::IsExecutable; // Trait for is_executable method

use super::helpers::create_temp_file;

// create temp file with a content, return path
fn create_temp_file_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (folder, content, extension) =
        extract_js_args!(args, ctx, Option<String>, Option<String>, Option<String>);

    match create_temp_file(folder.as_deref(), content.as_deref(), extension.as_deref()) {
        Ok(path) => Ok(JsValue::from(JsString::from(path))),
        Err(e) => Err(JsError::from(
            JsNativeError::error().with_message(format!("Error creating temp file: {}", e)),
        )),
    }
}

fn read_file_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let path = extract_js_args!(args, ctx, String);
    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(JsValue::from(JsString::from(content))),
        Err(e) => Err(JsError::from(
            JsNativeError::error().with_message(format!("Error reading file: {}", e)),
        )),
    }
}

fn write_file_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (path, content) = extract_js_args!(args, ctx, String, String);
    match std::fs::write(&path, content) {
        Ok(_) => Ok(JsValue::from(true)),
        Err(e) => Err(JsError::from(
            JsNativeError::error().with_message(format!("Error writing file: {}", e)),
        )),
    }
}

fn file_exists_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let path = extract_js_args!(args, ctx, String);
    let exists = std::path::Path::new(&path).exists();
    Ok(JsValue::from(exists))
}

fn file_is_executable_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let path = extract_js_args!(args, ctx, String);
    let is_executable = std::path::Path::new(&path).is_executable();
    Ok(JsValue::from(is_executable))
}

fn get_temp_dir_fn(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    let temp_dir = std::env::temp_dir();
    Ok(JsValue::from(JsString::from(temp_dir.to_string_lossy())))
}

fn get_home_dir_fn(_: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    let home_dir = if cfg!(target_os = "windows") {
        std::env::var_os("USERPROFILE")
    } else {
        std::env::var_os("HOME")
    };
    match home_dir {
        Some(home_path) => Ok(JsValue::from(JsString::from(home_path.to_string_lossy()))),
        None => Err(JsError::from(
            JsNativeError::error().with_message("Home directory not found"),
        )),
    }
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "File",
        [
            ("createTempFile", create_temp_file_fn, 3),
            ("read", read_file_fn, 1),
            ("write", write_file_fn, 2),
            ("exists", file_exists_fn, 1),
            ("isExecutable", file_is_executable_fn, 1),
            ("getTempDirectory", get_temp_dir_fn, 0),
            ("getHomeDirectory", get_home_dir_fn, 0),
        ]
    );
    Ok(())
}
