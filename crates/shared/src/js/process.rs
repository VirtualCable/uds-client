use std::env;
use std::path::PathBuf;

use anyhow::Result;
use boa_engine::{
    Context, JsResult, JsString, JsValue,
    error::{JsError, JsNativeError},
};
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
pub fn launch_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (app_path, app_args) = extract_js_args!(args, ctx, String, Vec<String>);

    log::debug!(
        "Executing application: {} with args: {:?}",
        app_path,
        app_args
    );
    let app_args_re: Vec<&str> = app_args.iter().map(|s| s.as_str()).collect();
    crate::system::launcher::launch(&app_path, app_args_re.as_slice(), None)
        .map(JsValue::from)
        .map_err(|e| JsError::from_native(JsNativeError::typ().with_message(format!("{}", e))))
}

pub fn is_running_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let process_id = extract_js_args!(args, ctx, u32);

    let running = crate::system::launcher::is_running(process_id);

    Ok(JsValue::from(running))
}

pub fn kill_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let process_id = extract_js_args!(args, ctx, u32);

    let stopped = crate::system::launcher::stop(process_id)
        .map(|_| JsValue::null())
        .map_err(|e| JsError::from_native(JsNativeError::typ().with_message(format!("{}", e))))?;

    Ok(stopped)
}

pub fn wait_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let process_id = extract_js_args!(args, ctx, u32);

    crate::system::launcher::wait(process_id)
        .map(|_| JsValue::null())
        .map_err(|e| JsError::from_native(JsNativeError::typ().with_message(format!("{}", e))))
}

pub fn wait_timeout_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (process_id, timeout_ms) = extract_js_args!(args, ctx, u32, u32);

    let timeout = std::time::Duration::from_millis(timeout_ms as u64);

    let triggered = crate::system::launcher::wait_timeout(process_id, timeout)
        .map_err(|e| JsError::from_native(JsNativeError::typ().with_message(format!("{}", e))))?;

    Ok(JsValue::from(triggered))
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "Process",
        // Sync functions
        [
            ("findExecutable", find_executable_fn, 2),
            ("launch", launch_fn, 2),
            ("isRunning", is_running_fn, 1),
            ("kill", kill_fn, 1),
            ("wait", wait_fn, 1),
            ("waitTimeout", wait_timeout_fn, 2),
        ],
        // Async functions, none here
        [],
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{exec_script, create_context};
    use super::*;

    use crate::log;
    use anyhow::Result;

    #[tokio::test]
    #[ignore = "Depends on system environment"]
    async fn test_find_executable() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context()?;
        // Register the process module
        register(&mut ctx)?;
        // Test finding an existing executable
        #[cfg(target_os = "windows")]
        let script = r#"
            let result = Process.findExecutable("cmd.exe", []);
            result;
        "#;
        #[cfg(not(target_os = "windows"))]
        let script = r#"
            let result = Process.findExecutable("bash");  // Second argument is optional
            result;
        "#;
        let result = exec_script(&mut ctx, script).await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        let result: String = result
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;

        log::info!("Found executable at: {}", result);

        assert!(!result.is_empty(), "Expected to find executable path");

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Depends on system environment"]
    async fn test_launch_is_running_stop() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context()?;

        // Register the process module
        register(&mut ctx)?;

        // Launch powershell on windows, ls on linux/mac
        #[cfg(target_os = "windows")]
        let script_launch = r#"
            let app_path = Process.findExecutable("powershell.exe", []);
            let handle = Process.launch(app_path, ["-NoExit", "-Command", "Start-Sleep -Seconds 6"]);
            handle;
        "#;
        #[cfg(not(target_os = "windows"))]
        let script_launch = r#"
            let handle = Process.launch("sleep", ["6"]);
            handle;
        "#;
        let result = exec_script(&mut ctx, script_launch).await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        // Wait a second to ensure the process starts
        std::thread::sleep(std::time::Duration::from_secs(1));

        let process_id: u32 = result
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;

        log::info!("Launched process with ID: {}", process_id);

        let script_is_running = r#"
            let isRunning = Process.isRunning(handle);
            isRunning;
        "#;
        let result_is_running = exec_script(&mut ctx, script_is_running).await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        let is_running: bool = result_is_running
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;

        log::info!("Process is running: {}", is_running);

        // Kill the process
        let script_kill = r#"
            Process.kill(handle);
            let finished = Process.waitTimeout(handle, 7000);
            finished;
        "#;
        let result = exec_script(&mut ctx, script_kill).await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        let finished: bool = result
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;
        log::info!("Process finished after kill: {}", finished);

        assert!(finished, "Expected process to finish after kill");

        Ok(())
    }
}
