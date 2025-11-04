use std::env;
use std::path::PathBuf;

use anyhow::Result;
use boa_engine::{Context, JsResult, JsString, JsValue};
use is_executable::IsExecutable; // Trait for is_executable method

use crate::log;

fn find_executable_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (app_name, extra_path) = extract_js_args!(args, ctx, String, Vec<String>);

    let mut search_paths: Vec<PathBuf> = Vec::new();

    if let Some(path_var) = env::var_os("PATH") {
        search_paths.extend(env::split_paths(&path_var));
    }

    // Append extra paths provided as argument
    search_paths.extend(extra_path.into_iter().map(PathBuf::from));

    log::debug!(
        "Searching for executable '{}' in paths: {:?}",
        app_name,
        search_paths
    );

    // look for the executable in the search paths
    let found = search_paths.iter().find_map(|dir| {
        let candidate = dir.join(&app_name);
        if candidate.is_executable() {
            Some(candidate)
        } else {
            None
        }
    });

    // Devolver resultado a JS
    if let Some(path) = found {
        Ok(JsValue::from(JsString::from(path.to_string_lossy())))
    } else {
        Ok(JsValue::null())
    }
}

// Execute app on background and returns app handle or error
pub fn execute_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (app_path, app_args) = extract_js_args!(args, ctx, String, Vec<String>);

    log::debug!(
        "Executing application: {} with args: {:?}",
        app_path,
        app_args
    );

    let mut command = std::process::Command::new(app_path);
    command.args(app_args);

    match command.spawn() {
        Ok(child) => Ok(JsValue::from(child.id())),
        Err(e) => Err(boa_engine::error::JsNativeError::range()
            .with_message(format!("Failed to execute application: {}", e))
            .into()),
    }
}

pub fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "Utils",
        [
            ("find_executable", find_executable_fn, 2),
            ("execute", execute_fn, 2),
        ]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::exec_script;
    use super::*;

    use crate::log;
    use anyhow::Result;
    use boa_engine::Context;

    #[test]
    #[ignore = "Depends on system environment"]
    fn test_find_executable() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = Context::default();
        // Register the process module
        register(&mut ctx)?;
        // Test finding an existing executable
        #[cfg(target_os = "windows")]
        let script = r#"
            let result = Utils.find_executable("cmd.exe", []);
            result;
        "#;
        #[cfg(not(target_os = "windows"))]
        let script = r#"
            let result = Utils.find_executable("bash");  // Second argument is optional
            result;
        "#;
        let result = exec_script(&mut ctx, script)
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        let result: String = result
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;

        log::info!("Found executable at: {}", result);

        assert!(!result.is_empty(), "Expected to find executable path");

        Ok(())
    }
}
