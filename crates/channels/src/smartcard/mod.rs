// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Smartcard integration handle for uds-client.
//!
//! Provides `SmartcardHandle` which implements the `SmartcardIntegration`
//! trait from the `rdp` crate. Uses an internal backend trait to allow
//! swapping between the emulated GIDS card and the native PC/SC backend.
//! When no real smartcard is available, `SmartcardHandle::new()` returns
//! `None` and the smartcard redirect is simply not enabled.

mod emulated;
mod native;

use std::time::Duration;

use pcsc::ffi::DWORD;
use rdp::integrations::smartcard::*;

use emulated::EmulatedBackend;
use native::NativeBackend;

// ---------------------------------------------------------------------------
// Internal Backend Trait
// ---------------------------------------------------------------------------

/// Internal backend trait — decouples `SmartcardHandle` from the actual
/// SCard implementation (dummy vs pcsc-lite).
trait SmartcardBackend: Send + Sync + std::fmt::Debug {
    fn establish_context(&self, scope: DWORD) -> Result<ScardContext, u32>;
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
        share_mode: DWORD,
        preferred_protocols: DWORD,
    ) -> Result<ConnectResult, u32>;
    fn disconnect(&self, handle: &ScardHandle, disposition: DWORD) -> Result<(), u32>;
    fn reconnect(
        &self,
        handle: &ScardHandle,
        share_mode: DWORD,
        preferred_protocols: DWORD,
        initialization: DWORD,
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
        control_code: DWORD,
        in_data: &[u8],
    ) -> Result<Vec<u8>, u32>;
    fn status(&self, handle: &ScardHandle) -> Result<ScardStatus, u32>;
    fn get_status_change(
        &self,
        ctx: &ScardContext,
        timeout: Duration,
        reader_states: &[ReaderStateIn],
    ) -> Result<(Vec<ReaderStateOut>, u32), u32>;
    fn cancel(&self, ctx: &ScardContext) -> Result<(), u32>;
    fn begin_transaction(&self, handle: &ScardHandle) -> Result<(), u32>;
    fn end_transaction(&self, handle: &ScardHandle, disposition: DWORD) -> Result<(), u32>;
    fn get_attrib(&self, handle: &ScardHandle, attr_id: DWORD) -> Result<Vec<u8>, u32>;
    fn set_attrib(&self, handle: &ScardHandle, attr_id: DWORD, data: &[u8]) -> Result<(), u32>;
    fn get_container_info(&self, ctx: &ScardContext, container_index: u8) -> Result<Vec<u8>, u32>;
    fn get_certificate(&self, ctx: &ScardContext) -> Result<Vec<u8>, u32>;
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
    /// Create a handle for the smartcard integration, or `None` when there is no
    /// real smartcard to redirect (emulated spec invalid / no PC/SC available).
    ///
    /// Priority:
    /// 1. `emulated` spec (from the RDP settings) → emulated backend, or
    ///    `None` (with a warning) if the spec is invalid.
    /// 2. `UDS_SMARTCARD_EMULATED=1` + `UDS_SMARTCARD_KEYS` (dev helper).
    /// 3. Native PC/SC backend (physical card), if available.
    pub fn new(emulated: Option<String>) -> Option<Self> {
        if let Some(spec) = emulated.as_deref() {
            return EmulatedBackend::from_spec(spec).map(|b| SmartcardHandle {
                backend: Box::new(b),
            });
        }
        if std::env::var("UDS_SMARTCARD_EMULATED").as_deref() == Ok("1") {
            return EmulatedBackend::try_from_env().map(|b| SmartcardHandle {
                backend: Box::new(b),
            });
        }
        let native = NativeBackend::new();
        if native.is_available() {
            Some(SmartcardHandle {
                backend: Box::new(native),
            })
        } else {
            log::debug!("smartcard: no native PC/SC backend available, no smartcard");
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Trait delegation
// ---------------------------------------------------------------------------

impl SmartcardIntegration for SmartcardHandle {
    fn establish_context(&self, scope: u32) -> Result<ScardContext, u32> {
        self.backend.establish_context(scope.into())
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
            .connect(ctx, reader, share_mode.into(), preferred_protocols.into())
    }

    fn disconnect(&self, handle: &ScardHandle, disposition: u32) -> Result<(), u32> {
        self.backend.disconnect(handle, disposition.into())
    }

    fn reconnect(
        &self,
        handle: &ScardHandle,
        share_mode: u32,
        preferred_protocols: u32,
        initialization: u32,
    ) -> Result<u32, u32> {
        self.backend.reconnect(
            handle,
            share_mode.into(),
            preferred_protocols.into(),
            initialization.into(),
        )
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
        self.backend.control(handle, control_code.into(), in_data)
    }

    fn status(&self, handle: &ScardHandle) -> Result<ScardStatus, u32> {
        self.backend.status(handle)
    }

    fn get_status_change(
        &self,
        ctx: &ScardContext,
        timeout: Duration,
        reader_states: &[ReaderStateIn],
    ) -> Result<(Vec<ReaderStateOut>, u32), u32> {
        self.backend.get_status_change(ctx, timeout, reader_states)
    }

    fn cancel(&self, ctx: &ScardContext) -> Result<(), u32> {
        self.backend.cancel(ctx)
    }

    fn begin_transaction(&self, handle: &ScardHandle) -> Result<(), u32> {
        self.backend.begin_transaction(handle)
    }

    fn end_transaction(&self, handle: &ScardHandle, disposition: u32) -> Result<(), u32> {
        self.backend.end_transaction(handle, disposition.into())
    }

    fn get_attrib(&self, handle: &ScardHandle, attr_id: u32) -> Result<Vec<u8>, u32> {
        self.backend.get_attrib(handle, attr_id.into())
    }

    fn set_attrib(&self, handle: &ScardHandle, attr_id: u32, data: &[u8]) -> Result<(), u32> {
        self.backend.set_attrib(handle, attr_id.into(), data)
    }

    fn get_container_info(&self, ctx: &ScardContext, container_index: u8) -> Result<Vec<u8>, u32> {
        self.backend.get_container_info(ctx, container_index)
    }

    fn get_certificate(&self, ctx: &ScardContext) -> Result<Vec<u8>, u32> {
        self.backend.get_certificate(ctx)
    }

    fn is_available(&self) -> bool {
        self.backend.is_available()
    }
}
