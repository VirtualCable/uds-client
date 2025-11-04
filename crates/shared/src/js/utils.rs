use anyhow::Result;
use boa_engine::{Context, JsResult, JsString, JsValue, error::JsNativeError};

use super::helpers;

#[cfg(target_os = "windows")]
use crate::system::{
    crypt_protect_data, read_hkcu_str, read_hklm_str, write_hkcu_dword, write_hkcu_str,
};

// windows_only: write to HKCU the key/value pair (string, string, string)
fn write_hkcu_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    #[cfg(not(target_os = "windows"))]
    return Err(boa_engine::error::JsError::type_error(
        "write_hkcu is only available on Windows",
    ));

    #[cfg(target_os = "windows")]
    {
        let (key, value_name, value_data) = extract_js_args!(args, ctx, String, String, String);

        write_hkcu_str(&key, &value_name, &value_data)
            .map_err(|e| JsNativeError::error().with_message(format!("Error: {}", e)))?;

        Ok(JsValue::undefined())
    }
}

fn write_hkcu_dword_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    #[cfg(not(target_os = "windows"))]
    return Err(boa_engine::error::JsError::type_error(
        "write_hkcu_dword is only available on Windows",
    ));

    #[cfg(target_os = "windows")]
    {
        let (key, value_name, value_data) = extract_js_args!(args, ctx, String, String, u32);

        write_hkcu_dword(&key, &value_name, value_data)
            .map_err(|e| JsNativeError::error().with_message(format!("Error: {}", e)))?;

        Ok(JsValue::undefined())
    }
}

// windows_only: read from HKCU the key/value pair. return string / error
fn read_hkcu_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    #[cfg(not(target_os = "windows"))]
    return Err(boa_engine::error::JsError::type_error(
        "read_hkcu is only available on Windows",
    ));

    #[cfg(target_os = "windows")]
    {
        let (key, value_name) = extract_js_args!(args, ctx, String, String);

        read_hkcu_str(&key, &value_name)
            .map_err(|e| JsNativeError::error().with_message(format!("Error: {}", e)))?;

        Ok(JsValue::undefined())
    }
}

// windows only: read HKLM key/value pair. return string / error
fn read_hklm_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    #[cfg(not(target_os = "windows"))]
    return Err(boa_engine::error::JsError::type_error(
        "read_hklm is only available on Windows",
    ));

    #[cfg(target_os = "windows")]
    {
        let (key, value_name) = extract_js_args!(args, ctx, String, String);

        let result = read_hklm_str(&key, &value_name)
            .map_err(|e| JsNativeError::error().with_message(format!("Error: {}", e)))?;

        Ok(JsValue::from(JsString::from(result)))
    }
}

// windows_only: crypt_protect_data(input: String), return encrypted string as base64 JsValue
fn crypt_protect_data_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    #[cfg(not(target_os = "windows"))]
    return Err(boa_engine::error::JsError::type_error(
        "crypt_protect_data is only available on Windows",
    ));

    #[cfg(target_os = "windows")]
    {
        let input = extract_js_args!(args, ctx, String);

        let encrypted = crypt_protect_data(&input)
            .map_err(|e| JsNativeError::error().with_message(format!("Error: {}", e)))?;

        Ok(JsValue::from(JsString::from(encrypted)))
    }
}

// test server (host: String, port: u16, timeout_ms: u64), return bool i
fn test_server_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (host, port, timeout_ms) = extract_js_args!(args, ctx, String, u16, u64);
    // If timeout_ms is 0, use a default value of 500ms
    let timeout_ms = if timeout_ms == 0 { 500 } else { timeout_ms };

    let result = helpers::test_server(&host, port, timeout_ms);
    Ok(JsValue::from(result))
}

// expandvars(input: String), return expanded string from env vars
fn expandvars_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let input = extract_js_args!(args, ctx, String);

    let expanded = helpers::expand_vars(&input).unwrap_or(input);
    Ok(JsValue::from(JsString::from(expanded)))
}

pub(super) fn register(ctx: &mut Context) -> Result<()> {
    register_js_module!(
        ctx,
        "Utils",
        [
            ("expandvars", expandvars_fn, 1),
            ("test_server", test_server_fn, 3),
            ("crypt_protect_data", crypt_protect_data_fn, 1),
            ("write_hkcu", write_hkcu_fn, 3),
            ("write_hkcu_dword", write_hkcu_dword_fn, 3),
            ("read_hkcu", read_hkcu_fn, 2),
            ("read_hklm", read_hklm_fn, 2),
        ]
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::exec_script;
    use super::*;
    use crate::log;
    use base64::{Engine as _, engine::general_purpose};

    use anyhow::Result;
    use boa_engine::Context;

    #[test]
    fn test_utils_log() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = Context::default();

        // Register the utils module
        register(&mut ctx)?;

        // Run a test script
        exec_script(
            &mut ctx,
            r#"
            Utils.log("info", "This works!");
            Utils.log("debug", "This is a test");
        "#,
        )
        .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        Ok(())
    }

    #[test]
    fn test_utils_expandvars() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = Context::default();

        // Register the utils module
        register(&mut ctx)?;

        unsafe { std::env::set_var("TEST_VALUE", "Test123") };
        unsafe { std::env::set_var("ANOTHER_VAR", "AnotherValue") };

        // Run a test script
        #[cfg(target_os = "windows")]
        let script = r#"
            let expanded = Utils.expandvars("Hello, %TEST_VALUE% %ANOTHER_VAR%!");
            expanded
        "#;
        #[cfg(not(target_os = "windows"))]
        let script = r#"
            let expanded = Utils.expandvars("Hello, ${TEST_VALUE} ${ANOTHER_VAR}!");
            expanded
        "#;

        let result = exec_script(&mut ctx, script)
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        // Verify the result
        assert_eq!(
            result,
            JsValue::from(JsString::from("Hello, Test123 AnotherValue!"))
        );

        Ok(())
    }

    #[cfg(test)]
    #[test]
    #[ignore = "Requires a server to access internet"]
    fn test_utils_test_server_works() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = Context::default();

        // Register the utils module
        register(&mut ctx)?;

        // Run a test script
        let script = r#"
            let isOpen = Utils.test_server("google.com", 80, 500);
            isOpen
        "#;

        let result = exec_script(&mut ctx, script)
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        // Verify the result
        assert_eq!(result, JsValue::from(true));

        Ok(())
    }

    #[cfg(test)]
    #[test]
    fn test_utils_test_server_fails() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = Context::default();

        // Register the utils module
        register(&mut ctx)?;

        // Run a test script
        let script = r#"
            let isOpen = Utils.test_server("invalid.host", 80, 500);
            isOpen
        "#;

        let result = exec_script(&mut ctx, script)
            .map_err(|e| anyhow::anyhow!("JavaScript execution error: {}", e))?;

        // Verify the result
        assert_eq!(result, JsValue::from(false));

        Ok(())
    }

    #[cfg(test)]
    #[test]
    #[cfg(target_os = "windows")]
    fn test_utils_crypt_protect_data() -> Result<()> {
        log::setup_logging("debug", log::LogType::Tests);
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = Context::default();
        // Register the utils module
        register(&mut ctx)?;

        // Run a test script
        let script = r#"
            let encrypted = Utils.crypt_protect_data("SensitiveData123");
            encrypted
        "#;

        let result = exec_script(&mut ctx, script)
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
}
