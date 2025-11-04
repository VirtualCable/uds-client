use std::{
    collections::HashMap,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU32},
    },
};

use super::{execute_app, trigger};
use crate::log;

#[derive(Debug, Clone)]
struct ProcessInfo {
    pub stop: trigger::Trigger,
}

impl ProcessInfo {
    pub fn new(stop: trigger::Trigger) -> Self {
        Self {
            stop,
        }
    }
}

static PROCESS_HANDLE_COUNTER: AtomicU32 = AtomicU32::new(1);
static PROCESS_INFOS: LazyLock<Mutex<HashMap<u32, ProcessInfo>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn register_process() -> (u32, trigger::Trigger) {
    let id = PROCESS_HANDLE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let trigger = trigger::Trigger::new();
    PROCESS_INFOS
        .lock()
        .unwrap()
        .insert(id, ProcessInfo::new(trigger.clone()));
    (id, trigger)
}

fn unregister_process(process_id: u32) {
    PROCESS_INFOS.lock().unwrap().remove(&process_id);
}

#[allow(dead_code)]
pub fn launch(application: &str, parameters: &[&str], cwd: Option<&str>) -> anyhow::Result<u32> {
    let (process_id, stop_trigger) = register_process();
    // Copy to owned strings to move into thread
    let application = application.to_string();
    let parameters: Vec<String> = parameters.iter().map(|s| s.to_string()).collect();
    let cwd = cwd.map(|s| s.to_string());

    std::thread::spawn(move || {
        // Get back the parameters as [&str]
        let params: Vec<&str> = parameters.iter().map(|s| s.as_str()).collect();
        let res = execute_app(&application, &params, Some(stop_trigger), cwd.as_deref());
        if let Err(e) = res {
            log::error!("Failed to execute app {}: {}", application, e);
        }
        unregister_process(process_id);
    });

    Ok(process_id)
}

#[allow(dead_code)]
pub fn is_running(process_id: u32) -> bool {
    if let Some(_info) = PROCESS_INFOS.lock().unwrap().get(&process_id) {
        true  // If the process info exists, we consider it running
    } else {
        false
    }
}

#[allow(dead_code)]
pub fn stop(process_id: u32) -> anyhow::Result<()> {
    if let Some(info) = PROCESS_INFOS.lock().unwrap().get(&process_id) {
        info.stop.set();
        Ok(())
    } else {
        Err(anyhow::anyhow!("Process ID {} not found", process_id))
    }
}
