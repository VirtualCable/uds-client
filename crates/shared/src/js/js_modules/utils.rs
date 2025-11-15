use anyhow::Result;
use boa_engine::{Context, JsResult, JsString, JsValue, error::JsNativeError};
use std::cell::RefCell;

use crate::js::helpers;

#[cfg(target_os = "windows")]
use crate::system::{
    crypt_protect_data, read_hkcu_str, read_hklm_str, write_hkcu_dword, write_hkcu_str,
};

// windows_only: write to HKCU the key/value pair (string, string, string)
fn write_hkcu_fn(_: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    #[cfg(not(target_os = "windows"))]
    return Err(JsNativeError::error()
        .with_message("write_hkcu is only available on Windows")
        .into());

    #[cfg(target_os = "windows")]
    {
        let (key, value_name, value_data) = extract_js_args!(_args, _ctx, String, String, String);

        write_hkcu_str(&key, &value_name, &value_data)
            .map_err(|e| JsNativeError::error().with_message(format!("Error: {}", e)))?;

        Ok(JsValue::undefined())
    }
}

fn write_hkcu_dword_fn(_: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    #[cfg(not(target_os = "windows"))]
    return Err(JsNativeError::error()
        .with_message("write_hkcu_dword is only available on Windows")
        .into());

    #[cfg(target_os = "windows")]
    {
        let (key, value_name, value_data) = extract_js_args!(_args, _ctx, String, String, u32);

        write_hkcu_dword(&key, &value_name, value_data)
            .map_err(|e| JsNativeError::error().with_message(format!("Error: {}", e)))?;

        Ok(JsValue::undefined())
    }
}

// windows_only: read from HKCU the key/value pair. return string / error
fn read_hkcu_fn(_: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    #[cfg(not(target_os = "windows"))]
    return Err(JsNativeError::error()
        .with_message("read_hkcu is only available on Windows")
        .into());

    #[cfg(target_os = "windows")]
    {
        let (key, value_name) = extract_js_args!(_args, _ctx, String, String);

        read_hkcu_str(&key, &value_name)
            .map_err(|e| JsNativeError::error().with_message(format!("Error: {}", e)))?;

        Ok(JsValue::undefined())
    }
}

// windows only: read HKLM key/value pair. return string / error
fn read_hklm_fn(_: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    #[cfg(not(target_os = "windows"))]
    return Err(JsNativeError::error()
        .with_message("read_hklm is only available on Windows")
        .into());

    #[cfg(target_os = "windows")]
    {
        let (key, value_name) = extract_js_args!(_args, _ctx, String, String);

        let result = read_hklm_str(&key, &value_name)
            .map_err(|e| JsNativeError::error().with_message(format!("Error: {}", e)))?;

        Ok(JsValue::from(JsString::from(result)))
    }
}

// windows_only: crypt_protect_data(input: String), return encrypted string as base64 JsValue
fn crypt_protect_data_fn(_: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    #[cfg(not(target_os = "windows"))]
    return Err(JsNativeError::error()
        .with_message("crypt_protect_data is only available on Windows")
        .into());

    #[cfg(target_os = "windows")]
    {
        let input = extract_js_args!(_args, _ctx, String);

        let encrypted = crypt_protect_data(&input)
            .map_err(|e| JsNativeError::error().with_message(format!("Error: {}", e)))?;

        Ok(JsValue::from(JsString::from(encrypted)))
    }
}

async fn test_server_fn(
    _: &JsValue,
    args: &[JsValue],
    ctx: &RefCell<&mut Context>,
) -> JsResult<JsValue> {
    let (host, port, timeout_ms) = {
        let mut ctx = ctx.borrow_mut();
        extract_js_args!(args, &mut ctx, String, u16, u64)
    };
    // If timeout_ms is 0, use a default value of 500ms
    let timeout_ms = if timeout_ms == 0 { 500 } else { timeout_ms };

    let result = helpers::test_server(&host, port, timeout_ms).await;
    Ok(JsValue::from(result))
}

fn expandvars_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let input = extract_js_args!(args, ctx, String);

    let expanded = helpers::expand_vars(&input).unwrap_or(input);
    Ok(JsValue::from(JsString::from(expanded)))
}

async fn sleep_fn(_: &JsValue, args: &[JsValue], ctx: &RefCell<&mut Context>) -> JsResult<JsValue> {
    let milliseconds = {
        let mut ctx = ctx.borrow_mut();
        extract_js_args!(args, &mut ctx, u64)
    };
    tokio::time::sleep(std::time::Duration::from_millis(milliseconds)).await;
    Ok(JsValue::undefined())
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "Utils",
        // Sync functions
        [
            ("expandVars", expandvars_fn, 1),
            ("cryptProtectData", crypt_protect_data_fn, 1),
            ("writeHkcu", write_hkcu_fn, 3),
            ("writeHkcuDword", write_hkcu_dword_fn, 3),
            ("readHkcu", read_hkcu_fn, 2),
            ("readHklm", read_hklm_fn, 2),
        ],
        // Async functions, test server can have some delay
        [("testServer", test_server_fn, 3), ("sleep", sleep_fn, 1),],
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        js::{create_context, exec_script_with_result},
        log,
    };

    use anyhow::Result;

    #[tokio::test]
    async fn test_utils_expandvars() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;

        // Register the utils module
        register(&mut ctx)?;

        unsafe { std::env::set_var("TEST_VALUE", "Test123") };
        unsafe { std::env::set_var("ANOTHER_VAR", "AnotherValue") };

        // Run a test script
        #[cfg(target_os = "windows")]
        let script = r#"
            let expanded = Utils.expandVars("Hello, %TEST_VALUE% %ANOTHER_VAR%!");
            expanded
        "#;
        #[cfg(not(target_os = "windows"))]
        let script = r#"
            let expanded = Utils.expandVars("Hello, ${TEST_VALUE} ${ANOTHER_VAR}!");
            expanded
        "#;

        let result = exec_script_with_result(&mut ctx, script)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        // Verify the result
        assert_eq!(
            result,
            JsValue::from(JsString::from("Hello, Test123 AnotherValue!"))
        );

        Ok(())
    }

    #[cfg(test)]
    #[tokio::test]
    #[ignore = "Requires a server to access internet"]
    async fn test_utils_test_server_works() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;

        // Register the utils module
        register(&mut ctx)?;
        // And the log
        super::super::logger::register(&mut ctx)?;

        // Run a test script
        // Note: Results from `Utils.testServer` are asynchronous
        // To get results, we need to use `.then` or `await` in the script mode (the one that can return a value)
        let script = r#"
            Utils.testServer("google.com", 80, 500).then(result => {
            Logger.debug("Test server result: " + result);
            return result;
        });
        "#;

        let result = exec_script_with_result(&mut ctx, script)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        // Verify the result
        assert_eq!(result, JsValue::from(true));

        Ok(())
    }

    #[cfg(test)]
    #[tokio::test]
    async fn test_utils_test_server_fails() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;

        // Register the utils module
        register(&mut ctx)?;
        // And log
        super::super::logger::register(&mut ctx)?;

        // Run a test script
        let script = r#"
            Utils.testServer("invalid.host", 80, 500).then(isOpen => {
            Logger.debug("Test server result: " + isOpen);
            return isOpen;
        });
        "#;

        let result = exec_script_with_result(&mut ctx, script)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        // Verify the result
        assert_eq!(result, JsValue::from(false));

        Ok(())
    }

    #[cfg(test)]
    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_utils_crypt_protect_data() -> Result<()> {
        use base64::{Engine as _, engine::general_purpose};
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;
        // Register the utils module
        register(&mut ctx)?;

        // Run a test script
        let script = r#"
            let encrypted = Utils.cryptProtectData("SensitiveData123");
            encrypted
        "#;

        let result = exec_script_with_result(&mut ctx, script)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        let result: String = result
            .try_js_into(&mut ctx)
            .map_err(|e| anyhow::anyhow!("Failed to convert result from JsValue: {}", e))?;

        log::info!("Encrypted result: {}", result);

        // Verify the result is not empty and is a string base64
        assert!(!result.is_empty());
        assert!(general_purpose::STANDARD.decode(result.as_str()).is_ok());

        Ok(())
    }

    #[tokio::test]
    async fn test_utils_sleep() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None)?;
        // Register the utils module
        register(&mut ctx)?;
        let start = std::time::Instant::now();
        // Run a test script
        let script = r#"
            Utils.sleep(1000).then(() => {
                // Sleep done
            });
        "#;
        let _result = exec_script_with_result(&mut ctx, script)
            .await
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;
        let duration = start.elapsed();
        // Verify that at least 1 second has passed
        assert!(duration.as_millis() >= 1000);
        Ok(())
    }
}
