use std::sync::{LazyLock, Mutex};

use crate::{log, system::launcher};

// Global waitable tasks
static WAITABLE_APPS: LazyLock<Mutex<Vec<u32>>> = LazyLock::new(|| Mutex::new(Vec::<u32>::new()));

static EARLY_UNLINKABLE_FILES: LazyLock<Mutex<Vec<String>>> =
    LazyLock::new(|| Mutex::new(Vec::<String>::new()));

static LATE_UNLINKABLE_FILES: LazyLock<Mutex<Vec<String>>> =
    LazyLock::new(|| Mutex::new(Vec::<String>::new()));

// add task to wait loop, initally, we will only watch this task, not the child processes
pub fn add_waitable_app(task_handle: u32) {
    let mut tasks = WAITABLE_APPS.lock().unwrap();
    tasks.push(task_handle);
}

// remove task from wait loop
pub fn remove_waitable_app(task_handle: u32) {
    let mut tasks = WAITABLE_APPS.lock().unwrap();
    if let Some(pos) = tasks.iter().position(|&x| x == task_handle) {
        tasks.remove(pos);
    }
}

// Wait for all registered apps to finish
pub fn wait_all_apps() {
    let apps = WAITABLE_APPS.lock().unwrap().clone();
    for app in apps {
        let res = launcher::wait(app);
        // If error, log but continue
        if res.is_err() {
            log::info!("App {} already exited", app);
        }
    }
}

// Add new file to deleter, on early or late phase
pub fn add_early_unlinkable_file(file_path: String) {
    let mut files = EARLY_UNLINKABLE_FILES.lock().unwrap();
    files.push(file_path);
}

pub fn unlink_early_files() {
    let files = EARLY_UNLINKABLE_FILES.lock().unwrap().clone();
    for file in files {
        let res = std::fs::remove_file(&file);
        // Log error but continue
        if let Err(e) = res {
            log::error!("Failed to unlink early file {}: {}", file, e);
        }
    }
}

pub fn add_late_unlinkable_file(file_path: String) {
    let mut files = LATE_UNLINKABLE_FILES.lock().unwrap();
    files.push(file_path);
}

pub fn unlink_late_files() {
    let files = LATE_UNLINKABLE_FILES.lock().unwrap().clone();
    for file in files {
        let res = std::fs::remove_file(&file);
        // Log error but continue
        if let Err(e) = res {
            log::error!("Failed to unlink late file {}: {}", file, e);
        }
    }
}

// Wait the time indicated, remove early unlinkable files, wait all apps, then remove late unlinkable files
pub fn wait_all_and_cleanup(timeout: std::time::Duration) {
    std::thread::sleep(timeout);
    unlink_early_files();
    wait_all_apps();
    unlink_late_files();
}
