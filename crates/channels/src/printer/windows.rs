// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Author: Andres Schumann, aschumann at virtualcable dot es

//! Windows print spooler backend (winspool.drv).
//!
//! Printers are announced with their own driver name: the RDP server renders
//! with that driver when it has it. The job keeps the queue's default
//! datatype, as FreeRDP does: forcing RAW breaks v4 (XPS) drivers such as
//! Microsoft Print to PDF, whose filter pipeline then fails and deletes the job.

use widestring::{U16CStr, U16CString};
use windows::Win32::Graphics::Printing::{
    AbortPrinter, ClosePrinter, DOC_INFO_1W, EndDocPrinter, EndPagePrinter, EnumPrintersW,
    GetDefaultPrinterW, OpenPrinterW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_HANDLE,
    PRINTER_INFO_2W, StartDocPrinterW, StartPagePrinter, WritePrinter,
};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Controls::Dialogs::{
    GetSaveFileNameW, OFN_EXPLORER, OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
    OPENFILENAMEW,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows::core::{PCWSTR, PWSTR};

use rdp::integrations::printer::PrinterDeviceInfo;

use super::{BackendJob, PrinterBackend, STATUS_UNSUCCESSFUL, SaveAsPdfJob};

#[derive(Debug, Default)]
pub(crate) struct WindowsBackend {
    // Driver of each announced printer, so a job does not enumerate them again.
    drivers: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl PrinterBackend for WindowsBackend {
    fn list_printers(&self) -> Vec<PrinterDeviceInfo> {
        let printers = enum_printers();
        if let Ok(mut drivers) = self.drivers.lock() {
            *drivers = printers.iter().cloned().collect();
        }
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
        let driver = self
            .drivers
            .lock()
            .ok()
            .and_then(|drivers| drivers.get(printer_name).cloned())
            .or_else(|| printer_driver(printer_name));
        if driver.is_some_and(|driver| super::is_print_to_pdf(&driver)) {
            return Ok(Box::new(SaveAsPdfJob::create(job_name, save_pdf_as)?));
        }
        let printer = Printer::open(printer_name).map_err(|err| {
            log::error!("printer: OpenPrinterW failed for '{printer_name}': {err}");
            STATUS_UNSUCCESSFUL
        })?;
        let mut doc_name = U16CString::from_str(job_name).map_err(|_| STATUS_UNSUCCESSFUL)?;
        let doc_info = DOC_INFO_1W {
            pDocName: PWSTR(doc_name.as_mut_ptr()),
            pOutputFile: PWSTR::null(),
            pDatatype: PWSTR::null(),
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

fn printer_driver(printer_name: &str) -> Option<String> {
    enum_printers()
        .into_iter()
        .find(|(name, _)| name == printer_name)
        .map(|(_, driver)| driver)
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

/// Asks where to save the PDF on its own thread, so the printer worker is not
/// blocked while the dialog is open.
fn save_pdf_as(temp: std::path::PathBuf, suggested_name: String) {
    let temp_for_error = temp.clone();
    let spawned = std::thread::Builder::new()
        .name("printer-save-pdf".into())
        .spawn(move || {
            let com =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
            let target = ask_save_path(&suggested_name);
            if com.is_ok() {
                unsafe { CoUninitialize() };
            }
            match target {
                Some(target) => move_file(&temp, &target),
                None => log::info!("printer: PDF not saved, the dialog was cancelled"),
            }
            let _ = std::fs::remove_file(&temp);
        });
    if let Err(err) = spawned {
        log::error!("printer: cannot open the save dialog: {err}");
        let _ = std::fs::remove_file(&temp_for_error);
    }
}

fn move_file(from: &std::path::Path, to: &std::path::Path) {
    // The temporary folder may be on another volume, where rename fails.
    let result = std::fs::rename(from, to).or_else(|_| {
        std::fs::copy(from, to)?;
        std::fs::remove_file(from)
    });
    match result {
        Ok(()) => log::info!("printer: PDF saved to {}", to.display()),
        Err(err) => log::error!("printer: saving the PDF to {} failed: {err}", to.display()),
    }
}

fn ask_save_path(suggested_name: &str) -> Option<std::path::PathBuf> {
    const MAX_PATH_CHARS: usize = 32_768;
    let mut buffer = vec![0u16; MAX_PATH_CHARS];
    let name: Vec<u16> = suggested_name.encode_utf16().collect();
    buffer[..name.len().min(MAX_PATH_CHARS - 1)]
        .copy_from_slice(&name[..name.len().min(MAX_PATH_CHARS - 1)]);
    let filter: Vec<u16> = "PDF\0*.pdf\0\0".encode_utf16().collect();
    let default_ext = U16CString::from_str("pdf").ok()?;
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        // The job comes from the session window: keep the dialog above it.
        hwndOwner: unsafe { GetForegroundWindow() },
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: MAX_PATH_CHARS as u32,
        lpstrDefExt: PCWSTR(default_ext.as_ptr()),
        Flags: OFN_EXPLORER | OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    if !unsafe { GetSaveFileNameW(&mut ofn) }.as_bool() {
        return None;
    }
    U16CStr::from_slice_truncate(&buffer)
        .ok()
        .map(|path| std::path::PathBuf::from(path.to_os_string()))
}
