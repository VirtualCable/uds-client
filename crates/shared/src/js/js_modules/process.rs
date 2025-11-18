use std::env;
use std::path::PathBuf;

use anyhow::Result;
use boa_engine::{
    Context, JsResult, JsString, JsValue,
    error::{JsError, JsNativeError},
    js_string,
    object::ObjectInitializer,
    property::Attribute,
    value::TryIntoJs,
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
    search_paths.extend(extra_path.clone().into_iter().map(PathBuf::from));

    log::debug!(
        "Searching for executable '{}' in PATH + {:?}",
        app_name,
        extra_path
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

// Executes and app, waits for it to finish, returns the output (stdout, stderr) as an array
async fn launch_and_wait_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &std::cell::RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let (app_path, app_args, mut timeout_ms) = {
        let mut ctx_borrow = ctx.borrow_mut();
        extract_js_args!(args, &mut *ctx_borrow, String, Vec<String>, u32)
    };
    if timeout_ms == 0 {
        timeout_ms = 30000; // Default to 30 seconds
    }

    log::debug!(
        "Running application: {} with args: {:?}",
        app_path,
        app_args
    );
    let app_args_re: Vec<&str> = app_args.iter().map(|s| s.as_str()).collect();
    // with timeout if set
    let output = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms as u64),
        tokio::process::Command::new(&app_path)
            .args(&app_args_re)
            .output(),
    )
    .await
    .map_err(|e| JsError::from_native(JsNativeError::typ().with_message(format!("{}", e))))?
    .map_err(|e| {
        JsError::from_native(
            JsNativeError::typ().with_message(format!("Failed to execute process: {}", e)),
        )
    })?;
    let result = {
        let mut ctx_borrow = ctx.borrow_mut();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout_js = stdout.try_into_js(*ctx_borrow)?;
        let stderr_js = stderr.try_into_js(*ctx_borrow)?;
        ObjectInitializer::new(*ctx_borrow)
            .property(js_string!("stdout"), stdout_js, Attribute::READONLY)
            .property(js_string!("stderr"), stderr_js, Attribute::READONLY)
            .build()
    };
    Ok(JsValue::from(result))
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

pub async fn wait_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &std::cell::RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let process_id = {
        let mut ctx_borrow = ctx.borrow_mut();
        extract_js_args!(args, &mut *ctx_borrow, u32)
    };

    crate::system::launcher::wait(process_id)
        .await
        .map(|_| JsValue::null())
        .map_err(|e| JsError::from_native(JsNativeError::typ().with_message(format!("{}", e))))
}

pub async fn wait_timeout_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &std::cell::RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let (process_id, timeout_ms) = {
        let mut ctx_borrow = ctx.borrow_mut();
        extract_js_args!(args, &mut *ctx_borrow, u32, u32)
    };

    let timeout = std::time::Duration::from_millis(timeout_ms as u64);

    let triggered = crate::system::launcher::wait_timeout(process_id, timeout)
        .await
        .map_err(|e| JsError::from_native(JsNativeError::typ().with_message(format!("{}", e))))?;

    Ok(JsValue::from(triggered))
}

pub async fn sleep_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &std::cell::RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let sleep_ms = {
        let mut ctx_borrow = ctx.borrow_mut();
        extract_js_args!(args, &mut *ctx_borrow, u32)
    };

    tokio::time::sleep(std::time::Duration::from_millis(sleep_ms as u64)).await;

    Ok(JsValue::null())
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
        ],
        // Async functions, none here
        [
            ("launchAndWait", launch_and_wait_fn, 3),
            ("wait", wait_fn, 1),
            ("waitTimeout", wait_timeout_fn, 2),
            ("sleep", sleep_fn, 1),
        ]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::js::{create_context, exec_script_with_result};

    use boa_engine::value::TryFromJs;

    use crate::log;
    use anyhow::Result;

    #[tokio::test]
    #[ignore = "Depends on system environment"]
    async fn test_find_executable() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;
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
        let result = exec_script_with_result(&mut ctx, script)
            .await
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
        let mut ctx = create_context(None)?;

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
        let result = exec_script_with_result(&mut ctx, script_launch)
            .await
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
        let result_is_running = exec_script_with_result(&mut ctx, script_is_running)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        let is_running: bool = result_is_running
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;

        log::info!("Process is running: {}", is_running);

        // Kill the process
        let script_wait = r#"
            Process.kill(handle);
            let finished = Process.waitTimeout(handle, 7000);
            finished;
        "#;
        let result = exec_script_with_result(&mut ctx, script_wait)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        let finished: bool = result
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;
        log::info!("Process finished after kill: {}", finished);

        assert!(finished, "Expected process to finish after kill");

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Depends on system environment"]
    async fn test_launch_and_wait() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;
        // Register the process module
        register(&mut ctx)?;
        // Test executing an application and waiting for output
        #[cfg(target_os = "windows")]
        let script = r#"
            let app_path = Process.findExecutable("cmd.exe", []);
            let result = Process.launchAndWait(app_path, ["/C", "echo Hello, World!"]);
            result;
        "#;
        #[cfg(not(target_os = "windows"))]
        let script = r#"
            let result = Process.launchAndWait("echo", ["Hello, World!"]);
            result;
        "#;
        let obj = exec_script_with_result(&mut ctx, script)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;
        let output = HashMap::<String, String>::try_from_js(&obj, &mut ctx);
        assert!(output.is_ok(), "Expected result to be an array");
        let output = output.unwrap();
        let stdout = output.get("stdout").cloned().unwrap_or_default();
        let stderr = output.get("stderr").cloned().unwrap_or_default();
        log::info!("ExecAndWait stdout: {}", stdout);
        log::info!("ExecAndWait stderr: {}", stderr);
        assert!(
            stdout.contains("Hello, World!"),
            "Expected stdout to contain 'Hello, World!'"
        );
        assert!(stderr.is_empty(), "Expected stderr to be empty");
        Ok(())
    }

    #[tokio::test]
    async fn test_launch_and_wait_non_existing_app() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;
        // Register the process module
        register(&mut ctx)?;
        // Test executing a non-existing application
        let script = r#"
            let result = Process.launchAndWait("non_existing_app_12345", []);
            result;
        "#;
        let result = exec_script_with_result(&mut ctx, script)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e));
        // Shoud return an error
        assert!(
            result.is_err(),
            "Expected error when executing non-existing app"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_wait_timeout_non_existing_process() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;
        // Register the process module
        register(&mut ctx)?;
        // Test waiting on a non-existing process
        let script = r#"
            let result = Process.waitTimeout(9999, 1000); // Assuming this PID does not exist
            result;
        "#;
        let result = exec_script_with_result(&mut ctx, script)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e));
        // Shoud return an error
        assert!(
            result.is_err(),
            "Expected error when waiting on non-existing process"
        );
        Ok(())
    }
}
