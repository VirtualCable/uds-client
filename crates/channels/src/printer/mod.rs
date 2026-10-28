// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com
// Author: Andres Schumann, aschumann at virtualcable dot es

//! Printer channel handle for uds-client.
//!
//! Provides `PrinterHandle` which implements the `PrinterIntegration`
//! trait from the `rdp` crate. It owns the job numbering and the table of
//! open jobs; the OS specific work (listing printers, talking to the
//! spooler) is done by a `PrinterBackend`.
//!
//! # Architecture / Roadmap (for developers / padawans)
//! Implement OS-specific backends under:
//! - `windows` (WinSpool RAW using winspool.drv) — done
//! - `linux/` (CUPS IPP client / spool)
//! - `macos/` (native CUPS)
//!
//! Refer to `doc/printer-redirection-plan.md` for full specifications.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use rdp::integrations::printer::{PrinterDeviceInfo, PrinterIntegration};

#[cfg(target_os = "windows")]
mod windows;

const STATUS_UNSUCCESSFUL: u32 = 0xC000_0001;
const STATUS_INVALID_HANDLE: u32 = 0xC000_0008;

/// A print job opened in the local spooler, as the backend sees it.
pub(crate) trait BackendJob: Send {
    /// Writes the whole chunk or fails.
    fn write(&mut self, data: &[u8]) -> Result<usize, u32>;
    fn finish(self: Box<Self>) -> Result<(), u32>;
    fn abort(self: Box<Self>);
}

/// OS specific side of printer redirection.
pub(crate) trait PrinterBackend: Send + Sync + std::fmt::Debug {
    fn list_printers(&self) -> Vec<PrinterDeviceInfo>;
    fn start_job(&self, printer_name: &str, job_name: &str) -> Result<Box<dyn BackendJob>, u32>;
}

/// An open job; emptied when it is finished or aborted.
type JobSlot = Arc<Mutex<Option<Box<dyn BackendJob>>>>;

pub struct PrinterHandle {
    backend: Option<Box<dyn PrinterBackend>>,
    // The RDP server echoes this id back as FileId on every write and close.
    next_job_id: AtomicU32,
    // Each job has its own lock so a slow spooler call on one printer does
    // not stall the worker threads of the others.
    jobs: Mutex<HashMap<u32, JobSlot>>,
}

impl std::fmt::Debug for PrinterHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrinterHandle")
            .field("backend", &self.backend)
            .field("open_jobs", &self.open_jobs())
            .finish()
    }
}

impl Default for PrinterHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl PrinterHandle {
    pub fn new() -> Self {
        Self::with_backend(platform_backend())
    }

    pub(crate) fn with_backend(backend: Option<Box<dyn PrinterBackend>>) -> Self {
        Self {
            backend,
            next_job_id: AtomicU32::new(1),
            jobs: Mutex::new(HashMap::new()),
        }
    }

    fn open_jobs(&self) -> usize {
        self.jobs.lock().map(|jobs| jobs.len()).unwrap_or(0)
    }

    fn find_job(&self, job_id: u32) -> Option<JobSlot> {
        self.jobs.lock().ok()?.get(&job_id).cloned()
    }

    fn take_job(&self, job_id: u32) -> Option<Box<dyn BackendJob>> {
        let slot = self.jobs.lock().ok()?.remove(&job_id)?;
        slot.lock().ok()?.take()
    }
}

#[cfg(target_os = "windows")]
fn platform_backend() -> Option<Box<dyn PrinterBackend>> {
    Some(Box::new(windows::WindowsBackend))
}

#[cfg(not(target_os = "windows"))]
fn platform_backend() -> Option<Box<dyn PrinterBackend>> {
    None
}

/// Builds the printers announced to the session from (name, driver, port).
/// Like FreeRDP, the first printer is the default when the OS has none.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn announce(
    printers: Vec<(String, String, Option<String>)>,
    default: Option<&str>,
) -> Vec<PrinterDeviceInfo> {
    let mut announced: Vec<PrinterDeviceInfo> = printers
        .into_iter()
        .enumerate()
        .map(
            |(index, (name, driver_name, port_name))| PrinterDeviceInfo {
                id: index as u32 + 1,
                is_default: default == Some(name.as_str()),
                name,
                driver_name,
                port_name,
            },
        )
        .collect();
    if !announced.iter().any(|printer| printer.is_default)
        && let Some(first) = announced.first_mut()
    {
        first.is_default = true;
    }
    announced
}

/// Calls `write` until the whole chunk is accepted: spoolers may take fewer
/// bytes than offered. A call that makes no progress is an error.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn write_all(
    data: &[u8],
    mut write: impl FnMut(&[u8]) -> Result<usize, u32>,
) -> Result<usize, u32> {
    let mut pending = data;
    while !pending.is_empty() {
        let written = write(pending)?;
        if written == 0 || written > pending.len() {
            return Err(STATUS_UNSUCCESSFUL);
        }
        pending = &pending[written..];
    }
    Ok(data.len())
}

impl PrinterIntegration for PrinterHandle {
    fn is_available(&self) -> bool {
        self.backend.is_some()
    }

    fn list_printers(&self) -> Vec<PrinterDeviceInfo> {
        self.backend
            .as_ref()
            .map(|backend| backend.list_printers())
            .unwrap_or_default()
    }

    fn create_job(&self, printer_name: &str) -> Result<u32, u32> {
        let backend = self.backend.as_ref().ok_or(STATUS_UNSUCCESSFUL)?;
        let job_id = self.next_job_id.fetch_add(1, Ordering::Relaxed);
        let job = backend.start_job(printer_name, &format!("UDS print job {job_id}"))?;
        self.jobs
            .lock()
            .map_err(|_| STATUS_UNSUCCESSFUL)?
            .insert(job_id, Arc::new(Mutex::new(Some(job))));
        Ok(job_id)
    }

    fn write_data(&self, job_id: u32, data: &[u8]) -> Result<usize, u32> {
        let slot = self.find_job(job_id).ok_or(STATUS_INVALID_HANDLE)?;
        let mut job = slot.lock().map_err(|_| STATUS_UNSUCCESSFUL)?;
        let result = job.as_mut().ok_or(STATUS_INVALID_HANDLE)?.write(data);
        if result.is_err()
            && let Some(failed) = job.take()
        {
            // The addin ignores errors on close, so a job that lost data must
            // not reach close_job and be printed incomplete. It is taken out
            // while still locked so a racing close finds the slot empty.
            drop(job);
            log::error!("printer: aborting job {job_id} after a failed write");
            failed.abort();
            if let Ok(mut jobs) = self.jobs.lock() {
                jobs.remove(&job_id);
            }
        }
        result
    }

    fn close_job(&self, job_id: u32) -> Result<(), u32> {
        self.take_job(job_id).ok_or(STATUS_INVALID_HANDLE)?.finish()
    }

    fn abort_job(&self, job_id: u32) {
        if let Some(job) = self.take_job(job_id) {
            job.abort();
        }
    }
}

impl Drop for PrinterHandle {
    // The addin never calls abort_job, so a session cut mid-print would leave
    // the job open in the local spooler.
    fn drop(&mut self) {
        let Ok(jobs) = self.jobs.get_mut() else {
            return;
        };
        for (job_id, slot) in jobs.drain() {
            if let Some(job) = slot.lock().ok().and_then(|mut job| job.take()) {
                log::warn!("printer: aborting job {job_id} left open at session end");
                job.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests;
