// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Smartcard integration handle for uds-client.
//!
//! Provides `SmartcardHandle` which implements the `SmartcardIntegration`
//! trait from the `rdp` crate. Uses an internal backend trait to allow
//! swapping between dummy (always available) and pcsc-lite (real hardware).
//!
//! Currently uses the dummy backend by default; pcsc-lite support will be
//! added in a future iteration.

mod dummy;
mod emulated;

use std::time::Duration;

use rdp::integrations::smartcard::*;

use dummy::DummyBackend;
use emulated::EmulatedBackend;

// ---------------------------------------------------------------------------
// Internal Backend Trait
// ---------------------------------------------------------------------------

/// Internal backend trait — decouples `SmartcardHandle` from the actual
/// SCard implementation (dummy vs pcsc-lite).
trait SmartcardBackend: Send + Sync + std::fmt::Debug {
    fn establish_context(&self, scope: u32) -> Result<ScardContext, u32>;
    fn release_context(&self, ctx: &ScardContext) -> Result<(), u32>;
    fn is_valid_context(&self, ctx: &ScardContext) -> bool;
    fn list_readers(
        &self,
        ctx: &ScardContext,
        groups: Option<&[String]>,
    ) -> Result<Vec<String>, u32>;
    fn connect(
        &self,
        ctx: &ScardContext,
        reader: &str,
        share_mode: u32,
        preferred_protocols: u32,
    ) -> Result<ConnectResult, u32>;
    fn disconnect(&self, handle: &ScardHandle, disposition: u32) -> Result<(), u32>;
    fn reconnect(
        &self,
        handle: &ScardHandle,
        share_mode: u32,
        preferred_protocols: u32,
        initialization: u32,
    ) -> Result<u32, u32>;
    fn transmit(
        &self,
        handle: &ScardHandle,
        send_pci: &ScardIORequest,
        data: &[u8],
    ) -> Result<TransmitResult, u32>;
    fn control(
        &self,
        handle: &ScardHandle,
        control_code: u32,
        in_data: &[u8],
    ) -> Result<Vec<u8>, u32>;
    fn status(&self, handle: &ScardHandle) -> Result<ScardStatus, u32>;
    fn get_status_change(
        &self,
        ctx: &ScardContext,
        timeout: Duration,
        reader_states: &[ReaderStateIn],
    ) -> Result<Vec<ReaderStateOut>, u32>;
    fn begin_transaction(&self, handle: &ScardHandle) -> Result<(), u32>;
    fn end_transaction(&self, handle: &ScardHandle, disposition: u32) -> Result<(), u32>;
    fn get_attrib(&self, handle: &ScardHandle, attr_id: u32) -> Result<Vec<u8>, u32>;
    fn set_attrib(&self, handle: &ScardHandle, attr_id: u32, data: &[u8]) -> Result<(), u32>;
    fn is_available(&self) -> bool;
}

// ---------------------------------------------------------------------------
// SmartcardHandle
// ---------------------------------------------------------------------------

/// Smartcard integration handle.
///
/// Wraps an internal backend (dummy by default, pcsc-lite later).
/// The dummy backend comes pre-configured with one virtual reader/card
/// for testing out of the box.
#[derive(Debug)]
pub struct SmartcardHandle {
    backend: Box<dyn SmartcardBackend>,
}

impl SmartcardHandle {
    /// Create a new handle, selecting the backend based on environment variables.
    ///
    /// - If `UDS_SMARTCARD_EMULATED=1` and cert/key paths are set, uses emulated backend
    /// - Otherwise, uses the dummy backend (always available)
    pub fn new() -> Self {
        let backend: Box<dyn SmartcardBackend> = if std::env::var("UDS_SMARTCARD_EMULATED")
            .as_deref()
            == Ok("1")
        {
            match EmulatedBackend::try_from_env() {
                Some(emulated) => Box::new(emulated),
                None => {
                    log::warn!(
                        "UDS_SMARTCARD_EMULATED=1 but failed to load cert/key, falling back to dummy"
                    );
                    Box::new(DummyBackend::new())
                }
            }
        } else {
            Box::new(DummyBackend::new())
        };
        SmartcardHandle { backend }
    }
}

impl Default for SmartcardHandle {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Trait delegation
// ---------------------------------------------------------------------------

impl SmartcardIntegration for SmartcardHandle {
    fn establish_context(&self, scope: u32) -> Result<ScardContext, u32> {
        self.backend.establish_context(scope)
    }

    fn release_context(&self, ctx: &ScardContext) -> Result<(), u32> {
        self.backend.release_context(ctx)
    }

    fn is_valid_context(&self, ctx: &ScardContext) -> bool {
        self.backend.is_valid_context(ctx)
    }

    fn list_readers(
        &self,
        ctx: &ScardContext,
        groups: Option<&[String]>,
    ) -> Result<Vec<String>, u32> {
        self.backend.list_readers(ctx, groups)
    }

    fn connect(
        &self,
        ctx: &ScardContext,
        reader: &str,
        share_mode: u32,
        preferred_protocols: u32,
    ) -> Result<ConnectResult, u32> {
        self.backend
            .connect(ctx, reader, share_mode, preferred_protocols)
    }

    fn disconnect(&self, handle: &ScardHandle, disposition: u32) -> Result<(), u32> {
        self.backend.disconnect(handle, disposition)
    }

    fn reconnect(
        &self,
        handle: &ScardHandle,
        share_mode: u32,
        preferred_protocols: u32,
        initialization: u32,
    ) -> Result<u32, u32> {
        self.backend
            .reconnect(handle, share_mode, preferred_protocols, initialization)
    }

    fn transmit(
        &self,
        handle: &ScardHandle,
        send_pci: &ScardIORequest,
        data: &[u8],
    ) -> Result<TransmitResult, u32> {
        self.backend.transmit(handle, send_pci, data)
    }

    fn control(
        &self,
        handle: &ScardHandle,
        control_code: u32,
        in_data: &[u8],
    ) -> Result<Vec<u8>, u32> {
        self.backend.control(handle, control_code, in_data)
    }

    fn status(&self, handle: &ScardHandle) -> Result<ScardStatus, u32> {
        self.backend.status(handle)
    }

    fn get_status_change(
        &self,
        ctx: &ScardContext,
        timeout: Duration,
        reader_states: &[ReaderStateIn],
    ) -> Result<Vec<ReaderStateOut>, u32> {
        self.backend.get_status_change(ctx, timeout, reader_states)
    }

    fn begin_transaction(&self, handle: &ScardHandle) -> Result<(), u32> {
        self.backend.begin_transaction(handle)
    }

    fn end_transaction(&self, handle: &ScardHandle, disposition: u32) -> Result<(), u32> {
        self.backend.end_transaction(handle, disposition)
    }

    fn get_attrib(&self, handle: &ScardHandle, attr_id: u32) -> Result<Vec<u8>, u32> {
        self.backend.get_attrib(handle, attr_id)
    }

    fn set_attrib(&self, handle: &ScardHandle, attr_id: u32, data: &[u8]) -> Result<(), u32> {
        self.backend.set_attrib(handle, attr_id, data)
    }

    fn is_available(&self) -> bool {
        self.backend.is_available()
    }
}
