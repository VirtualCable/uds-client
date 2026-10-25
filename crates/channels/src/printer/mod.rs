// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Printer channel handle for uds-client.
//!
//! Provides `PrinterHandle` which implements the `PrinterIntegration`
//! trait from the `rdp` crate.
//!
//! # Architecture / Roadmap (for developers / padawans)
//! Implement OS-specific backends under:
//! - `windows/` (WinSpool RAW / XPS_PASS using winspool.drv)
//! - `linux/` (CUPS IPP client / spool)
//! - `macos/` (native CUPS)
//!
//! Refer to `doc/printer-redirection-plan.md` for full specifications.

use rdp::integrations::printer::{PrinterDeviceInfo, PrinterIntegration};

#[derive(Debug)]
pub struct PrinterHandle {
    // OS-specific backend handles will go here
}

impl Default for PrinterHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl PrinterHandle {
    pub fn new() -> Self {
        Self {}
    }
}

impl PrinterIntegration for PrinterHandle {
    fn is_available(&self) -> bool {
        // TBD: Return true when OS printer backend is initialized
        false
    }

    fn list_printers(&self) -> Vec<PrinterDeviceInfo> {
        // TBD: Enumerate local OS printers
        Vec::new()
    }

    fn create_job(&self, _printer_name: &str) -> Result<u32, u32> {
        // TBD: Start document job in OS spooler
        Err(0xC000_0001) // STATUS_UNSUCCESSFUL
    }

    fn write_data(&self, _job_id: u32, _data: &[u8]) -> Result<usize, u32> {
        // TBD: Write raw/xps chunk to OS spooler
        Err(0xC000_0001)
    }

    fn close_job(&self, _job_id: u32) -> Result<(), u32> {
        // TBD: Finalize document in OS spooler
        Err(0xC000_0001)
    }

    fn abort_job(&self, _job_id: u32) {
        // TBD: Abort document in OS spooler
    }
}
