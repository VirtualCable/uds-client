// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
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
use std::sync::{LazyLock, Mutex};

use crate::{
    log,
    system::{launcher::is_running, trigger::Trigger},
    tunnel::is_any_tunnel_active,
};

// Global waitable tasks
static WAITABLE_APPS: LazyLock<Mutex<Vec<u32>>> = LazyLock::new(|| Mutex::new(Vec::<u32>::new()));

static EARLY_UNLINKABLE_FILES: LazyLock<Mutex<Vec<String>>> =
    LazyLock::new(|| Mutex::new(Vec::<String>::new()));

static LATE_UNLINKABLE_FILES: LazyLock<Mutex<Vec<String>>> =
    LazyLock::new(|| Mutex::new(Vec::<String>::new()));

static INTERNAL_RDP_RUNNING: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));

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
pub async fn wait_all_apps(stop: Trigger) {
    loop {
        let all_done = {
            let tasks = WAITABLE_APPS.lock().unwrap();
            tasks.iter().all(|id| !is_running(*id))
        };
        if all_done
            || stop
                .async_wait_timeout(std::time::Duration::from_secs(2))
                .await
        {
            break;
        }
    }
}

pub async fn wait_all_tunnels(stop: Trigger) {
    loop {
        if !is_any_tunnel_active()
            || stop
                .async_wait_timeout(std::time::Duration::from_secs(2))
                .await
        {
            break;
        }
    }
}

pub fn mark_internal_rdp_as_running() {
    log::debug!("Marking internal RDP client as running");
    let mut running = INTERNAL_RDP_RUNNING.lock().unwrap();
    *running = true;
}

pub fn mark_internal_rdp_as_not_running() {
    log::debug!("Marking internal RDP client as not running");
    let mut running = INTERNAL_RDP_RUNNING.lock().unwrap();
    *running = false;
}

pub fn is_internal_rdp_running() -> bool {
    let running = INTERNAL_RDP_RUNNING.lock().unwrap();
    *running
}

async fn wait_internal_rdp(stop: Trigger) {
    loop {
        if !is_internal_rdp_running()
            || stop
                .async_wait_timeout(std::time::Duration::from_secs(2))
                .await
        {
            break;
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
pub async fn wait_all_and_cleanup(timeout: std::time::Duration, stop: Trigger) {
    stop.async_wait_timeout(timeout).await;
    unlink_early_files();

    // Wait all apps to finish, or until stop is set
    // give stop as we do no need anymore it ownership
    wait_all_apps(stop.clone()).await;

    // Wait internal RDP client to finish if any
    wait_internal_rdp(stop.clone()).await;

    // Also for tunnels. On linux/macOS, the apps may run on background but tunnels may remain
    // so we wait for tunnels separately
    wait_all_tunnels(stop).await;

    unlink_late_files();
}
