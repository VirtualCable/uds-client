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
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use rdp::integrations::printer::{PrinterDeviceInfo, PrinterIntegration};

#[cfg(target_os = "windows")]
mod windows;

const STATUS_UNSUCCESSFUL: u32 = 0xC000_0001;
const STATUS_INVALID_HANDLE: u32 = 0xC000_0008;

/// A print job opened in the local spooler, as the backend sees it.
pub(crate) trait BackendJob: Send {
    fn write(&mut self, data: &[u8]) -> Result<usize, u32>;
    fn finish(self: Box<Self>) -> Result<(), u32>;
    fn abort(self: Box<Self>);
}

/// OS specific side of printer redirection.
pub(crate) trait PrinterBackend: Send + Sync + std::fmt::Debug {
    fn list_printers(&self) -> Vec<PrinterDeviceInfo>;
    fn start_job(&self, printer_name: &str, job_name: &str) -> Result<Box<dyn BackendJob>, u32>;
}

pub struct PrinterHandle {
    backend: Option<Box<dyn PrinterBackend>>,
    // The RDP server echoes this id back as FileId on every write and close.
    next_job_id: AtomicU32,
    jobs: Mutex<HashMap<u32, Box<dyn BackendJob>>>,
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

    fn take_job(&self, job_id: u32) -> Option<Box<dyn BackendJob>> {
        self.jobs.lock().ok()?.remove(&job_id)
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
            .insert(job_id, job);
        Ok(job_id)
    }

    fn write_data(&self, job_id: u32, data: &[u8]) -> Result<usize, u32> {
        let mut jobs = self.jobs.lock().map_err(|_| STATUS_UNSUCCESSFUL)?;
        let job = jobs.get_mut(&job_id).ok_or(STATUS_INVALID_HANDLE)?;
        job.write(data)
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
        for (job_id, job) in jobs.drain() {
            log::warn!("printer: aborting job {job_id} left open at session end");
            job.abort();
        }
    }
}

#[cfg(test)]
mod tests;
