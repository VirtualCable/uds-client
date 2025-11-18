// Copyright (c) 2025 Virtual Cable S.L.U.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without modification,
// are permitted provided that the following conditions are met:
//
//    * Redistributions of source code must retain the above copyright notice,
//      this list of conditions and the following disclaimer.
//    * Redistributions in binary form must reproduce the above copyright notice,
//      this list of conditions and the following disclaimer in the documentation
//      and/or other materials provided with the distribution.
//    * Neither the name of Virtual Cable S.L.U. nor the names of its contributors
//      may be used to endorse or promote products derived from this software
//      without specific prior written permission.
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
/*!
Author: Adolfo Gómez, dkmaster at dkmon dot com
*/
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    backtrace::Backtrace,
    panic,
    path::PathBuf,
    sync::OnceLock,
};
use tracing_log::log;
use tracing_subscriber::{
    EnvFilter, Layer, Registry, fmt, layer::SubscriberExt, reload, util::SubscriberInitExt,
};

// Reexport to avoid using crate names for tracing
pub use tracing::{debug, error, info, trace, warn};

static LOGGER_INIT: OnceLock<()> = OnceLock::new();
static RELOAD_HANDLE: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

struct RotatingWriter {
    path: PathBuf,
    max_size: u64,    // Max size in bytes before rotation
    max_files: usize, // Number of rotations to keep
}

impl RotatingWriter {
    fn rotate_if_needed(&self) -> io::Result<()> {
        if let Ok(meta) = fs::metadata(&self.path)
            && meta.len() >= self.max_size
        {
            // Remove last if needed
            if self.max_files > 1 {
                let last = self.path.with_extension(format!("log.{}", self.max_files));
                let _ = fs::remove_file(&last);
                // Rename in reverse order
                for i in (1..self.max_files).rev() {
                    let src = self.path.with_extension(format!("log.{}", i));
                    let dst = self.path.with_extension(format!("log.{}", i + 1));
                    let _ = fs::rename(&src, &dst);
                }
                // Rename current to .log.1
                let rotated = self.path.with_extension("log.1");
                let _ = fs::rename(&self.path, rotated);
            } else {
                // if max_files is 1, just remove current
                let _ = fs::remove_file(&self.path);
            }
        }
        Ok(())
    }
}

impl<'a> fmt::MakeWriter<'a> for RotatingWriter {
    type Writer = fs::File;

    fn make_writer(&'a self) -> Self::Writer {
        // Rotate if needed
        let _ = self.rotate_if_needed();
        // Always open in append mode, creating it if it doesn't exist
        // If self.path cannot be opened, try with one in temp dir
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .unwrap_or_else(|_e| {
                let temp_path = std::env::temp_dir().join("udsactor-fallback.log");
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&temp_path)
                    .unwrap_or_else(|e| panic!("Failed to open log file {:?}: {}", temp_path, e))
            })
    }
}

#[derive(PartialEq)]
pub enum LogType {
    Launcher,
    Tests,
}

impl std::fmt::Display for LogType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogType::Launcher => write!(f, "launcher"),
            LogType::Tests => write!(f, "launcher-tests"),
        }
    }
}

// Our log system wil also hook panics to log them
pub fn setup_panic_hook() {
    panic::set_hook(Box::new(|info| {
        let temp_log = std::env::temp_dir().join("udslauncher-panic.log");
        log::error!("Panic occurred, writing details to {:?}", temp_log);
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&temp_log)
            .unwrap();

        // Try to get message info
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Non-string panic payload".to_string()
        };

        // Location
        let loc = if let Some(location) = info.location() {
            format!("{}:{}", location.file(), location.line())
        } else {
            "unknown location".to_string()
        };

        // Backtrace
        let bt = Backtrace::capture();

        writeln!(f, "Panic occurred at {}: {}", loc, msg).ok();
        writeln!(f, "Backtrace:\n{:?}", bt).ok();

        error!("Guru Meditation (😕): {} at {}", msg, loc);
        error!("Backtrace:\n{:?}", bt);
        // Exit process
        std::process::exit(1);
    }));
}


pub fn setup_logging(level: &str, log_type: LogType) {
    let (level_key, log_path, use_datetime, log_name) = (
        format!(
            "UDSCLIENT_{}_LOG_LEVEL",
            log_type.to_string().to_uppercase()
        ),
        format!("UDSCLIENT_{}_LOG_PATH", log_type.to_string().to_uppercase()),
        format!(
            "UDSCLIENT_{}_LOG_USE_DATETIME",
            log_type.to_string().to_uppercase()
        ),
        format!("uds-{}", log_type.to_string().to_lowercase()),
    );

    // To keep compat with old behavior, if .uds-debug-on is on temp or user home, set level to debug
    let level = if std::path::Path::new(&std::env::temp_dir().join(".uds-debug-on")).exists()
        || std::path::Path::new(
            &std::env::home_dir()
                .unwrap_or_default()
                .join(".uds-debug-on"),
        )
        .exists()
    {
        "debug".to_string()
    } else {
        // Only if .uds-debug-on is not present, get level from env var or use default passed
        std::env::var(level_key).unwrap_or_else(|_| level.to_string())
    };

    let log_path =
        std::env::var(log_path).unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into());
    let use_datetime: bool = std::env::var(use_datetime)
        .unwrap_or_else(|_| "false".into())
        .to_lowercase()
        .parse()
        .unwrap_or(false);

    let log_name = if use_datetime {
        let computer_name = hostname::get()
            .unwrap_or_else(|_| "unknown".into())
            .to_string_lossy()
            .into_owned();
        let dt = chrono::Local::now();
        format!(
            "{}-{}-{}",
            log_name,
            computer_name,
            dt.format("%Y%m%d-%H%M%S")
        )
    } else {
        log_name.to_string()
    } + ".log";

    LOGGER_INIT.get_or_init(|| {
        let env_filter = EnvFilter::new(level.clone());
        let (reload_layer, handle) = reload::Layer::<EnvFilter, Registry>::new(env_filter);

        let _ = RELOAD_HANDLE.set(handle);

        let main_layer = fmt::layer()
            .with_writer(RotatingWriter {
                path: std::path::Path::new(&log_path).join(log_name),
                max_size: 16 * 1024 * 1024, // 16 MB
                max_files: 2,
            })
            .with_ansi(false)
            .with_target(true)
            .with_level(true)
            .with_thread_ids(level == "debug" || level == "trace")
            .with_filter(reload_layer);

        #[cfg(debug_assertions)]
        let main_layer = main_layer.and_then(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_target(true)
                .with_level(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true)
                .with_filter(EnvFilter::new("debug")),
        );

        tracing_subscriber::registry()
            .with(main_layer)
            .try_init()
            .ok();

        // Setup panic hook, not if testing
        if log_type != LogType::Tests {
            setup_panic_hook();
        }
    });
}

pub fn set_log_level(level: &str) {
    // Note: Changing log level at runtime is not directly supported by tracing_subscriber.
    // This is a workaround by re-initializing the subscriber with the new level.
    if let Some(handle) = RELOAD_HANDLE.get() {
        let new_filter = EnvFilter::new(level);
        if let Err(e) = handle.modify(|f| *f = new_filter) {
            eprintln!("Failed to reload log level: {}", e);
        }
    } else {
        eprintln!("Logger not initialized yet");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore] // Ignored because it requires Windows service environment
    fn test_logging_on_network_path() {
        unsafe { std::env::set_var("UDSACTOR_TESTS_LOG_PATH", r"\\172.27.1.45\shared") }
        setup_logging("debug", LogType::Tests);
        info!("This is a test log entry on network path");
        debug!("Debug entry");
        warn!("Warning entry");
        error!("Error entry");
        trace!("Trace entry");
    }

    #[test]
    fn test_logging_on_default_path() {
        setup_logging("debug", LogType::Tests);
        info!("This is a test log entry on default path");
        debug!("Debug entry");
        warn!("Warning entry");
        error!("Error entry");
        trace!("Trace entry");
    }

    #[test]
    fn test_logging_with_datetime() {
        unsafe {
            std::env::set_var("UDSACTOR_TESTS_LOG_PATH", std::env::temp_dir());
            std::env::set_var("UDSACTOR_TESTS_LOG_USE_DATETIME", "true");
        }
        setup_logging("debug", LogType::Tests);
        info!("This is a test log entry with datetime in filename");
        debug!("Debug entry");
        warn!("Warning entry");
        error!("Error entry");
        trace!("Trace entry");
    }

    #[test]
    #[ignore] // Ignored because it generates a lot of log data on console
    fn test_logging_rotation() {
        let temp_dir = std::env::temp_dir();
        unsafe { std::env::set_var("UDSACTOR_TESTS_LOG_PATH", &temp_dir) }
        setup_logging("debug", LogType::Tests);
        let log_file = temp_dir.join("udsactor-tests.log");
        // Write enough logs to exceed 16MB
        for i in 0..20000 {
            info!("Log entry number: {} - {}", i, "A".repeat(1024)); // Each entry ~1KB
        }
        // Check if log file exists
        assert!(log_file.exists());
        // Check if rotated file exists
        let rotated_file = temp_dir.join("udsactor-tests.log.1");
        assert!(rotated_file.exists()); // Rotated file should exist
        // Check if log file has been rotated
        let meta = fs::metadata(&log_file).unwrap();
        assert!(meta.len() < 16 * 1024 * 1024); // Current log file should be less than 16MB
    }
}
