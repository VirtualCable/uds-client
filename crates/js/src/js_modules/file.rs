// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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
use boa_engine::{
    Context, JsResult, JsString, JsValue,
    error::{JsError, JsNativeError},
};

use is_executable::IsExecutable; // Trait for is_executable method

use crate::helpers::create_temp_file;

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

fn is_directory_fn(_: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let path = extract_js_args!(args, ctx, String);
    let is_directory = std::path::Path::new(&path).is_dir();
    Ok(JsValue::from(is_directory))
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
            ("isDirectory", is_directory_fn, 1),
            ("getTempDirectory", get_temp_dir_fn, 0),
            ("getHomeDirectory", get_home_dir_fn, 0),
        ],
        []
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use boa_engine::js_string;

    use super::*;
    use shared::log;
    use crate::{create_context, exec_script_with_result};

    #[tokio::test]
    async fn test_file_module() {
        log::setup_logging("debug", log::LogType::Tests);
        let mut ctx = create_context(None).unwrap();
        register(&mut ctx).unwrap();

        // Test createTempFile
        let script = r#"
            const tempFilePath = File.createTempFile(null, "Hello, World!", "txt");
            const content = File.read(tempFilePath);
            File.write(tempFilePath, "New Content");
            const newContent = File.read(tempFilePath);
            const exists = File.exists(tempFilePath);
            const isExecutable = File.isExecutable(tempFilePath);
            const isDirectory = File.isDirectory(tempFilePath);
            const tempDir = File.getTempDirectory();
            const homeDir = File.getHomeDirectory();
            ({
                tempFilePath,
                content,
                newContent,
                exists,
                isExecutable,
                isDirectory,
                tempDir,
                homeDir
            });
        "#;

        let result = exec_script_with_result(&mut ctx, script).await.unwrap();

        let obj = result.as_object().unwrap();

        let temp_file_path: String = obj
            .get(js_string!("tempFilePath"), &mut ctx)
            .unwrap()
            .try_js_into(&mut ctx)
            .unwrap();
        assert!(!temp_file_path.is_empty());
        log::info!("Temp file created at: {}", temp_file_path);

        let content: String = obj
            .get(js_string!("content"), &mut ctx)
            .unwrap()
            .try_js_into(&mut ctx)
            .unwrap();
        assert_eq!(content, "Hello, World!");

        let new_content: String = obj
            .get(js_string!("newContent"), &mut ctx)
            .unwrap()
            .try_js_into(&mut ctx)
            .unwrap();
        assert_eq!(new_content, "New Content");

        let exists: bool = obj
            .get(js_string!("exists"), &mut ctx)
            .unwrap()
            .try_js_into(&mut ctx)
            .unwrap();
        assert!(exists);

        let is_executable: bool = obj
            .get(js_string!("isExecutable"), &mut ctx)
            .unwrap()
            .try_js_into(&mut ctx)
            .unwrap();
        assert!(!is_executable);

        let is_directory: bool = obj
            .get(js_string!("isDirectory"), &mut ctx)
            .unwrap()
            .try_js_into(&mut ctx)
            .unwrap();
        assert!(!is_directory);

        let temp_dir: String = obj
            .get(js_string!("tempDir"), &mut ctx)
            .unwrap()
            .try_js_into(&mut ctx)
            .unwrap();
        assert!(!temp_dir.is_empty());

        let home_dir: String = obj
            .get(js_string!("homeDir"), &mut ctx)
            .unwrap()
            .try_js_into(&mut ctx)
            .unwrap();
        assert!(!home_dir.is_empty());
    }
}
