// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Author: Andres Schumann, aschumann at virtualcable dot es

//! CUPS backend, with libcups loaded at run time.
//!
//! The job is spooled to a private temporary file and handed to CUPS in one
//! `cupsPrintFile` call when the server closes it, so CUPS never holds a
//! job that is still being written or that lost data.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::fs::File;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Arc;

use rdp::integrations::printer::PrinterDeviceInfo;

use super::{BackendJob, PrinterBackend, STATUS_UNSUCCESSFUL};

const LIBCUPS: &str = "libcups.so.2";

/// The server renders the job with this PostScript driver, the same one
/// FreeRDP announces for CUPS queues; CUPS then converts the PostScript for
/// the real printer.
pub(crate) const ANNOUNCED_DRIVER: &str = "MS Publisher Imagesetter";

#[repr(C)]
pub(crate) struct CupsDest {
    pub(crate) name: *mut c_char,
    pub(crate) instance: *mut c_char,
    pub(crate) is_default: c_int,
    pub(crate) num_options: c_int,
    pub(crate) options: *mut c_void,
}

type GetDests = unsafe extern "C" fn(*mut *mut CupsDest) -> c_int;
type FreeDests = unsafe extern "C" fn(c_int, *mut CupsDest);
pub(crate) type PrintFile = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    *const c_char,
    c_int,
    *const c_void,
) -> c_int;
pub(crate) type LastErrorString = unsafe extern "C" fn() -> *const c_char;

pub(crate) struct Cups {
    get_dests: GetDests,
    free_dests: FreeDests,
    pub(crate) print_file: PrintFile,
    pub(crate) last_error: LastErrorString,
    // Keeps the functions above valid.
    _library: Option<libloading::Library>,
}

impl Cups {
    fn load() -> Option<Self> {
        let library = unsafe { libloading::Library::new(LIBCUPS) }
            .map_err(|err| log::warn!("printer: cannot load {LIBCUPS}: {err}"))
            .ok()?;
        Some(Self {
            get_dests: symbol(&library, b"cupsGetDests\0")?,
            free_dests: symbol(&library, b"cupsFreeDests\0")?,
            print_file: symbol(&library, b"cupsPrintFile\0")?,
            last_error: symbol(&library, b"cupsLastErrorString\0")?,
            _library: Some(library),
        })
    }

    fn last_error_text(&self) -> String {
        let text = unsafe { (self.last_error)() };
        if text.is_null() {
            return "unknown error".into();
        }
        unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(test)]
impl Cups {
    pub(crate) fn with_print_file(print_file: PrintFile, last_error: LastErrorString) -> Self {
        unsafe extern "C" fn no_dests(_: *mut *mut CupsDest) -> c_int {
            0
        }
        unsafe extern "C" fn free_nothing(_: c_int, _: *mut CupsDest) {}
        Self {
            get_dests: no_dests,
            free_dests: free_nothing,
            print_file,
            last_error,
            _library: None,
        }
    }
}

fn symbol<T: Copy>(library: &libloading::Library, name: &[u8]) -> Option<T> {
    match unsafe { library.get::<T>(name) } {
        Ok(found) => Some(*found),
        Err(err) => {
            log::warn!(
                "printer: {LIBCUPS} has no {}: {err}",
                String::from_utf8_lossy(name)
            );
            None
        }
    }
}

pub(crate) struct CupsBackend {
    cups: Arc<Cups>,
}

impl std::fmt::Debug for CupsBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CupsBackend")
    }
}

impl CupsBackend {
    pub(crate) fn load() -> Option<Self> {
        Cups::load().map(|cups| Self {
            cups: Arc::new(cups),
        })
    }
}

impl PrinterBackend for CupsBackend {
    fn list_printers(&self) -> Vec<PrinterDeviceInfo> {
        let mut dests: *mut CupsDest = std::ptr::null_mut();
        let count = unsafe { (self.cups.get_dests)(&mut dests) };
        if dests.is_null() {
            return Vec::new();
        }
        let listed = if count > 0 {
            unsafe { queues(std::slice::from_raw_parts(dests, count as usize)) }
        } else {
            (Vec::new(), None)
        };
        unsafe { (self.cups.free_dests)(count, dests) };
        let (printers, default) = listed;
        super::announce(printers, default.as_deref())
    }

    fn start_job(&self, printer_name: &str, job_name: &str) -> Result<Box<dyn BackendJob>, u32> {
        Ok(Box::new(CupsJob::create(
            self.cups.clone(),
            printer_name,
            job_name,
        )?))
    }
}

/// Queues as (name, driver) plus the default one. Instances (`queue/instance`)
/// are option presets of a queue already listed, so they are skipped.
///
/// # Safety
/// Every `name` and `instance` must be null or a valid C string.
pub(crate) unsafe fn queues(dests: &[CupsDest]) -> (Vec<(String, String)>, Option<String>) {
    let mut printers = Vec::new();
    let mut default = None;
    for dest in dests {
        if dest.name.is_null() || !dest.instance.is_null() {
            continue;
        }
        let name = unsafe { CStr::from_ptr(dest.name) }
            .to_string_lossy()
            .into_owned();
        if dest.is_default != 0 {
            default = Some(name.clone());
        }
        printers.push((name, ANNOUNCED_DRIVER.to_string()));
    }
    (printers, default)
}

pub(crate) struct CupsJob {
    cups: Arc<Cups>,
    file: File,
    path: PathBuf,
    printer: CString,
    title: CString,
}

impl CupsJob {
    pub(crate) fn create(cups: Arc<Cups>, printer_name: &str, job_name: &str) -> Result<Self, u32> {
        let printer = CString::new(printer_name).map_err(|_| STATUS_UNSUCCESSFUL)?;
        let title = CString::new(job_name).map_err(|_| STATUS_UNSUCCESSFUL)?;
        let path = std::env::temp_dir().join(format!(
            "uds-print-{}-{}.prn",
            std::process::id(),
            rand::random::<u64>()
        ));
        // The document may be confidential: only the user can read the spool file.
        let file = File::options()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|err| {
                log::error!("printer: cannot create {}: {err}", path.display());
                STATUS_UNSUCCESSFUL
            })?;
        Ok(Self {
            cups,
            file,
            path,
            printer,
            title,
        })
    }

    #[cfg(test)]
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn submit(&self) -> Result<(), u32> {
        let size = self
            .file
            .metadata()
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if size == 0 {
            log::warn!(
                "printer: job for '{}' is empty, not printed",
                self.printer_name()
            );
            return Ok(());
        }
        let path =
            CString::new(self.path.as_os_str().as_bytes()).map_err(|_| STATUS_UNSUCCESSFUL)?;
        let job_id = unsafe {
            (self.cups.print_file)(
                self.printer.as_ptr(),
                path.as_ptr(),
                self.title.as_ptr(),
                0,
                std::ptr::null(),
            )
        };
        if job_id == 0 {
            log::error!(
                "printer: cupsPrintFile failed for '{}': {}",
                self.printer_name(),
                self.cups.last_error_text()
            );
            return Err(STATUS_UNSUCCESSFUL);
        }
        log::info!(
            "printer: sent {size} bytes to '{}' as CUPS job {job_id}",
            self.printer_name()
        );
        Ok(())
    }

    fn printer_name(&self) -> std::borrow::Cow<'_, str> {
        self.printer.to_string_lossy()
    }
}

impl BackendJob for CupsJob {
    fn write(&mut self, data: &[u8]) -> Result<usize, u32> {
        self.file.write_all(data).map_err(|err| {
            log::error!("printer: writing {} failed: {err}", self.path.display());
            STATUS_UNSUCCESSFUL
        })?;
        Ok(data.len())
    }

    fn finish(self: Box<Self>) -> Result<(), u32> {
        if let Err(err) = self.file.sync_all() {
            log::error!("printer: writing {} failed: {err}", self.path.display());
            return Err(STATUS_UNSUCCESSFUL);
        }
        self.submit()
    }

    fn abort(self: Box<Self>) {}
}

impl Drop for CupsJob {
    fn drop(&mut self) {
        // cupsPrintFile has already sent the whole file when it returns.
        let _ = std::fs::remove_file(&self.path);
    }
}
