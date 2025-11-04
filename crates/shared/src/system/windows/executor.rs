use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[allow(unused_imports)]
use windows::{
    Win32::{
        Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{
            JobObjects::AssignProcessToJobObject,
            Threading::{
                CREATE_SUSPENDED, CreateEventW, CreateProcessW, GetExitCodeProcess, INFINITE,
                PROCESS_INFORMATION, ResumeThread, STARTUPINFOW, SetEvent, WaitForMultipleObjects,
            },
        },
    },
    core::{PCWSTR, PWSTR},
};

use crate::log;

use super::{
    super::trigger,
    event,
    jobs::{create_job_object, terminate_job, wait_for_job},
    safe::SafeHandle,
};

#[derive(Clone)]
struct MonitorGuard {
    keep_running: Arc<AtomicBool>,
}

impl MonitorGuard {
    fn new() -> Self {
        Self {
            keep_running: Arc::new(AtomicBool::new(true)),
        }
    }

    fn is_set(&self) -> bool {
        self.keep_running.load(Ordering::Relaxed)
    }
}

impl Drop for MonitorGuard {
    fn drop(&mut self) {
        self.keep_running.store(false, Ordering::Relaxed);
    }
}

struct ProcessInfo {
    process: SafeHandle,
    thread: SafeHandle,
    pid: u32,
    #[allow(dead_code)]
    tid: u32,
}

pub fn execute_app(
    application: &str,
    parameters: &[&str],
    stop: Option<trigger::Trigger>,
    cwd: Option<&str>,
) -> anyhow::Result<()> {
    let application_as_param = "\"".to_string() + application.trim_matches('"') + "\"";

    let mut cleaned_params = application_as_param;

    if !parameters.is_empty() {
        // Ensure parameters are trimmed and joined with a space
        cleaned_params += " ";
        // If space is in parameters, we need to quote them
        cleaned_params += &parameters
            .iter()
            .map(|p| {
                let trimmed = p.trim();
                if trimmed.contains(' ') {
                    // Remove existing quotes and add new ones
                    "\"".to_string() + trimmed.trim_matches('"') + "\""
                } else {
                    trimmed.to_string()
                }
            })
            .collect::<Vec<String>>()
            .join(" ");
    }

    // Pre-check: does the executable exist?
    if !std::path::Path::new(application).exists() {
        log::error!("Executable does not exist: {}", application);
        return Err(anyhow::anyhow!(
            "Executable does not exist: {}",
            application
        ));
    }

    // Setup STARTUPINFO
    let pi: ProcessInfo = {
        let startup_info = STARTUPINFOW::default();

        // Convert strings to UTF-16
        let app_utf16 = widestring::U16CString::from_str_truncate(application);
        let mut params_utf16 = widestring::U16CString::from_str_truncate(cleaned_params);

        let mut pi = PROCESS_INFORMATION::default();
        let folder_utf16 = widestring::U16CString::from_str_truncate(cwd.unwrap_or(""));

        log::debug!("Creating process for application: {}", application,);

        unsafe {
            CreateProcessW(
                PCWSTR(app_utf16.as_ptr()),
                Some(PWSTR(params_utf16.as_mut_ptr())),
                None,
                None,
                false,
                CREATE_SUSPENDED,
                None,
                if folder_utf16.is_empty() {
                    PCWSTR::null()
                } else {
                    PCWSTR(folder_utf16.as_ptr())
                },
                &startup_info,
                &mut pi,
            )
            .map_err(|_e| {
                // Get the error code and the error message
                let error_code = windows::core::Error::from_thread().code();
                let error_message = windows::core::Error::from_thread().message();
                log::error!(
                    "Failed to create process: {} (Error code: {}, Message: {})",
                    application,
                    error_code,
                    error_message
                );
                anyhow::anyhow!(
                    "Failed to create process: {} (Error code: {}, Message: {})",
                    application,
                    error_code,
                    error_message
                )
            })?
        };

        ProcessInfo {
            process: SafeHandle::new(pi.hProcess),
            thread: SafeHandle::new(pi.hThread),
            pid: pi.dwProcessId,
            tid: pi.dwThreadId,
        }
    };
    let job: SafeHandle = create_job_object();

    unsafe {
        AssignProcessToJobObject(job.get(), pi.process.get())
            .map_err(|e| anyhow::anyhow!("Failed to assign process to job object: {:?}", e))?;
        ResumeThread(pi.thread.get());
    }

    log::debug!(
        "Process created with ID: {}, Handle: {:?}",
        pi.pid,
        pi.process
    );

    let keep_running = MonitorGuard::new();
    let stop_event = event::Event::new();

    if let Some(stop) = stop {
        log::debug!("Stop notifier provided, will monitor for stop signal");

        // On a thread, wait for stop trigger and if set, signal the stop_event
        std::thread::spawn({
            let stop_event = stop_event.clone();
            let stop = stop.clone();
            let keep_running = keep_running.clone();
            move || {
                log::debug!("Trigger wait thread started");
                while keep_running.is_set() {
                    if stop.wait_timeout(std::time::Duration::from_millis(300)) {
                        log::debug!("Trigger activated, signaling stop_event");
                        stop_event.signal();
                        break;
                    }
                }
            }
        });
    }

    // Waitable handles, application process and stop event
    let wait_result = {
        let handles = vec![pi.process.get(), stop_event.get().get()];
        unsafe { WaitForMultipleObjects(&handles, false, INFINITE) }
    };

    log::debug!("WaitForMultipleObjects returned: {:?}", wait_result);

    // If the wait result is the first handle (the process handle), it means the process exited
    // And if the wait result is the second handle (the event handle), it means the stop notifier was triggered
    if wait_result != WAIT_OBJECT_0 {
        // That is WAIT_OBJECT_0 + 1 or WAIT_TIMEOUT, that is, event_handler
        log::debug!("Stop triggered, killing process");

        if let Err(e) = terminate_job(job.clone()) {
            log::error!("Failed to terminate job {}: {:?}", job, e);
        } else {
            log::debug!("Process terminated successfully");
        }
        stop_event.signal();
    } else {
        log::debug!("Main app exited. Waiting for respawned processes...");
        wait_for_job(job.clone(), stop_event.clone())?;
    }

    // Get the exit code of the process
    let mut exit_code = 0;
    unsafe { GetExitCodeProcess(pi.process.get(), &mut exit_code)? };
    // If the exit code is not 0, log a warning
    if exit_code != 0 {
        log::warn!("Process exited with code {}", exit_code);
    }

    stop_event.signal();

    log::debug!("All done");
    Result::Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use rand::Rng;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

    /// Helper function to run exec_wait_application with a temp file and custom window state.
    fn run_exec_wait_application_with_temp_file() -> (anyhow::Result<()>, PathBuf) {
        log::setup_logging("debug", log::LogType::Tests);
        let folder_name = "C:\\Windows\\System32";
        let temp_dir = env::temp_dir();
        let random_suffix: String = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(8)
            .map(char::from)
            .collect();
        let temp_file = temp_dir.join(format!("test_exec_wait_application_{}.txt", random_suffix));
        let temp_file_str = temp_file.to_string_lossy();
        let cmd = format!("New-Item -Path '{}' -ItemType File -Force; Start-Sleep -Seconds 1", temp_file_str);
        let parameters = [
            "-Command",
            &cmd,
        ];
        let application = r"C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe";
        let result = execute_app(application, &parameters, None, Some(folder_name));
        (result, temp_file)
    }

    #[test]
    fn test_exec_wait_application_creates_temp_file() {
        let (result, temp_file) = run_exec_wait_application_with_temp_file();
        assert!(
            result.is_ok(),
            "Failed to execute PowerShell to create temp file: {:?}",
            result.err()
        );
        assert!(
            temp_file.exists(),
            "Temp file was not created by PowerShell script"
        );
        let _ = fs::remove_file(&temp_file);
    }

    #[test]
    fn test_exec_wait_application_invalid_path() {
        log::setup_logging("debug", log::LogType::Tests);
        let folder_name = "C:\\";
        let application = r"C:\\Path\\To\\NonExistentApp.exe";
        let result = execute_app(application, &[], None, Some(folder_name));
        assert!(
            result.is_err(),
            "Expected error for invalid application path, got: {:?}",
            result
        );
    }

    #[test]
    fn test_exec_wait_application_stop_notifier() {
        log::setup_logging("debug", log::LogType::Tests);
        let stop = trigger::Trigger::new();
        let folder_name = "C:\\";
        let application = r"c:\\windows\\notepad.exe";
        let handle = thread::spawn({
            let stop = stop.clone();
            move || execute_app(application, &[], Some(stop), Some(folder_name))
        });
        thread::sleep(Duration::from_millis(400));
        assert!(
            !handle.is_finished(),
            "Application thread should be running before stop_notifier is triggered"
        );
        assert!(
            !stop.is_set(),
            "Stop notifier should not be set before triggering"
        );
        stop.set();
        let result = handle.join().unwrap();
        assert!(
            result.is_ok(),
            "Application should exit early when stop_notifier is triggered: {:?}",
            result.err()
        );
    }
}
