// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Author: Andres Schumann, aschumann at virtualcable dot es

//! Windows print spooler backend (winspool.drv).
//!
//! Printers are announced with their own driver name: the RDP server renders
//! with that driver when it has it, and the data that arrives is already in
//! the printer language, so it is handed to the spooler as RAW.

use widestring::{U16CStr, U16CString};
use windows::Win32::Graphics::Printing::{
    AbortPrinter, ClosePrinter, DOC_INFO_1W, EndDocPrinter, EndPagePrinter, EnumPrintersW,
    GetDefaultPrinterW, OpenPrinterW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_HANDLE,
    PRINTER_INFO_2W, StartDocPrinterW, StartPagePrinter, WritePrinter,
};
use windows::core::{PCWSTR, PWSTR};

use rdp::integrations::printer::PrinterDeviceInfo;

use super::{BackendJob, PrinterBackend, STATUS_UNSUCCESSFUL};

#[derive(Debug)]
pub(crate) struct WindowsBackend;

impl PrinterBackend for WindowsBackend {
    fn list_printers(&self) -> Vec<PrinterDeviceInfo> {
        let printers = enum_printers();
        let openable = super::keep_openable(printers, |name| match Printer::open(name) {
            Ok(_) => true,
            Err(err) => {
                log::warn!("printer: skipping '{name}', it cannot be opened: {err}");
                false
            }
        });
        super::announce(openable, default_printer().as_deref())
    }

    fn start_job(&self, printer_name: &str, job_name: &str) -> Result<Box<dyn BackendJob>, u32> {
        let printer = Printer::open(printer_name).map_err(|err| {
            log::error!("printer: OpenPrinterW failed for '{printer_name}': {err}");
            STATUS_UNSUCCESSFUL
        })?;
        let mut doc_name = U16CString::from_str(job_name).map_err(|_| STATUS_UNSUCCESSFUL)?;
        let mut datatype = U16CString::from_str("RAW").map_err(|_| STATUS_UNSUCCESSFUL)?;
        let doc_info = DOC_INFO_1W {
            pDocName: PWSTR(doc_name.as_mut_ptr()),
            pOutputFile: PWSTR::null(),
            pDatatype: PWSTR(datatype.as_mut_ptr()),
        };
        if unsafe { StartDocPrinterW(printer.0, 1, &doc_info) } == 0 {
            log::error!(
                "printer: StartDocPrinterW failed for '{printer_name}': {}",
                last_error()
            );
            return Err(STATUS_UNSUCCESSFUL);
        }
        if !unsafe { StartPagePrinter(printer.0) }.as_bool() {
            log::error!(
                "printer: StartPagePrinter failed for '{printer_name}': {}",
                last_error()
            );
            let _ = unsafe { AbortPrinter(printer.0) };
            return Err(STATUS_UNSUCCESSFUL);
        }
        Ok(Box::new(SpoolerJob { printer }))
    }
}

/// An open printer; closed when dropped.
struct Printer(PRINTER_HANDLE);

// The handle is only used by the thread that owns the job at any given time.
unsafe impl Send for Printer {}

impl Printer {
    fn open(name: &str) -> windows::core::Result<Self> {
        let name = U16CString::from_str(name).map_err(|_| windows::core::Error::empty())?;
        let mut handle = PRINTER_HANDLE::default();
        unsafe { OpenPrinterW(PCWSTR(name.as_ptr()), &mut handle, None) }?;
        Ok(Self(handle))
    }
}

impl Drop for Printer {
    fn drop(&mut self) {
        if let Err(err) = unsafe { ClosePrinter(self.0) } {
            log::warn!("printer: ClosePrinter failed: {err}");
        }
    }
}

struct SpoolerJob {
    printer: Printer,
}

impl BackendJob for SpoolerJob {
    fn write(&mut self, data: &[u8]) -> Result<usize, u32> {
        super::write_all(data, |pending| {
            let len = u32::try_from(pending.len()).map_err(|_| STATUS_UNSUCCESSFUL)?;
            let mut written = 0u32;
            let ok =
                unsafe { WritePrinter(self.printer.0, pending.as_ptr().cast(), len, &mut written) }
                    .as_bool();
            if !ok {
                log::error!("printer: WritePrinter failed: {}", last_error());
                return Err(STATUS_UNSUCCESSFUL);
            }
            Ok(written as usize)
        })
    }

    fn finish(self: Box<Self>) -> Result<(), u32> {
        let page_ok = unsafe { EndPagePrinter(self.printer.0) }.as_bool();
        let doc_ok = unsafe { EndDocPrinter(self.printer.0) }.as_bool();
        if page_ok && doc_ok {
            Ok(())
        } else {
            log::error!("printer: ending the job failed: {}", last_error());
            Err(STATUS_UNSUCCESSFUL)
        }
    }

    fn abort(self: Box<Self>) {
        if !unsafe { AbortPrinter(self.printer.0) }.as_bool() {
            log::warn!("printer: AbortPrinter failed: {}", last_error());
        }
    }
}

fn last_error() -> windows::core::Error {
    windows::core::Error::from_thread()
}

fn default_printer() -> Option<String> {
    let mut len = 0u32;
    let _ = unsafe { GetDefaultPrinterW(None, &mut len) };
    if len == 0 {
        return None;
    }
    let mut buffer = vec![0u16; len as usize];
    if !unsafe { GetDefaultPrinterW(Some(PWSTR(buffer.as_mut_ptr())), &mut len) }.as_bool() {
        return None;
    }
    U16CStr::from_slice_truncate(&buffer)
        .ok()
        .map(|name| name.to_string_lossy())
}

/// Local and connected printers as (name, driver).
fn enum_printers() -> Vec<(String, String)> {
    let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    let (mut needed, mut returned) = (0u32, 0u32);
    // The first call only reports the buffer size and always fails with ERROR_INSUFFICIENT_BUFFER.
    let _ = unsafe { EnumPrintersW(flags, PCWSTR::null(), 2, None, &mut needed, &mut returned) };
    if needed == 0 {
        return Vec::new();
    }
    // u64 elements keep the buffer aligned for PRINTER_INFO_2W.
    let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(buffer.as_mut_ptr().cast::<u8>(), needed as usize)
    };
    if let Err(err) = unsafe {
        EnumPrintersW(
            flags,
            PCWSTR::null(),
            2,
            Some(bytes),
            &mut needed,
            &mut returned,
        )
    } {
        log::error!("printer: EnumPrintersW failed: {err}");
        return Vec::new();
    }
    let infos = unsafe {
        std::slice::from_raw_parts(buffer.as_ptr().cast::<PRINTER_INFO_2W>(), returned as usize)
    };
    infos
        .iter()
        .filter_map(|info| {
            let name = pwstr_to_string(info.pPrinterName)?;
            let driver = pwstr_to_string(info.pDriverName)?;
            Some((name, driver))
        })
        .collect()
}

fn pwstr_to_string(value: PWSTR) -> Option<String> {
    if value.is_null() {
        return None;
    }
    unsafe { value.to_string() }.ok()
}
