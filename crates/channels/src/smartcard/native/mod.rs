// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

pub(crate) mod helpers;
pub(crate) mod types;

use std::time::Duration;

use rdp::integrations::smartcard::*;

use super::SmartcardBackend;
use helpers::*;
use types::NativeRegistry;

#[derive(Debug)]
pub(crate) struct NativeBackend {
    registry: NativeRegistry,
}

impl NativeBackend {
    pub fn new() -> Self {
        NativeBackend {
            registry: NativeRegistry::new(),
        }
    }
}

impl SmartcardBackend for NativeBackend {
    fn establish_context(&self, _scope: u32) -> Result<ScardContext, u32> {
        let pcsc_ctx = pcsc::Context::establish(pcsc::Scope::System).map_err(pcsc_error_to_u32)?;
        let sc_ctx = ScardContext::new();
        let mut contexts = self.registry.contexts.write().unwrap();
        contexts.insert(sc_ctx.raw(), pcsc_ctx);
        Ok(sc_ctx)
    }

    fn release_context(&self, ctx: &ScardContext) -> Result<(), u32> {
        let mut contexts = self.registry.contexts.write().unwrap();
        contexts.remove(&ctx.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;
        Ok(())
    }

    fn is_valid_context(&self, ctx: &ScardContext) -> bool {
        let contexts = self.registry.contexts.read().unwrap();
        contexts.contains_key(&ctx.raw())
    }

    fn list_readers(
        &self,
        ctx: &ScardContext,
        _groups: Option<&[String]>,
    ) -> Result<Vec<String>, u32> {
        let contexts = self.registry.contexts.read().unwrap();
        let pcsc_ctx = contexts.get(&ctx.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let mut readers_buf = [0u8; 4096];
        let readers = match pcsc_ctx.list_readers(&mut readers_buf) {
            Ok(r) => r,
            Err(pcsc::Error::NoReadersAvailable) => return Ok(Vec::new()),
            Err(e) => return Err(pcsc_error_to_u32(e)),
        };

        let mut result = Vec::new();
        for reader in readers {
            if let Ok(s) = reader.to_str() {
                result.push(s.to_string());
            }
        }
        Ok(result)
    }

    fn connect(
        &self,
        ctx: &ScardContext,
        reader: &str,
        share_mode: u32,
        preferred_protocols: u32,
    ) -> Result<ConnectResult, u32> {
        let contexts = self.registry.contexts.read().unwrap();
        let pcsc_ctx = contexts.get(&ctx.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let share = u32_to_share_mode(share_mode)?;
        let protos = u32_to_protocols(preferred_protocols);

        let reader_c = std::ffi::CString::new(reader).map_err(|_| SCARD_E_INVALID_PARAMETER)?;
        let card = pcsc_ctx
            .connect(&reader_c, share, protos)
            .map_err(pcsc_error_to_u32)?;

        let mut r_buf = [0u8; 256];
        let mut a_buf = [0u8; 36];
        let active_proto = match card.status2(&mut r_buf, &mut a_buf) {
            Ok(status) => protocol_to_u32(status.protocol()),
            Err(_) => SCARD_PROTOCOL_T1, // Default fallback
        };

        let handle = ScardHandle::new(active_proto);
        let mut cards = self.registry.cards.write().unwrap();
        cards.insert(handle.raw(), (card, reader.to_string()));

        Ok(ConnectResult {
            handle,
            active_protocol: active_proto,
        })
    }

    fn disconnect(&self, handle: &ScardHandle, disposition: u32) -> Result<(), u32> {
        let disp = u32_to_disposition(disposition)?;
        let mut cards = self.registry.cards.write().unwrap();
        if let Some((card, _)) = cards.remove(&handle.raw()) {
            card.disconnect(disp)
                .map_err(|(_, err)| pcsc_error_to_u32(err))?;
            Ok(())
        } else {
            Err(SCARD_E_INVALID_HANDLE)
        }
    }

    fn reconnect(
        &self,
        handle: &ScardHandle,
        share_mode: u32,
        preferred_protocols: u32,
        initialization: u32,
    ) -> Result<u32, u32> {
        let share = u32_to_share_mode(share_mode)?;
        let protos = u32_to_protocols(preferred_protocols);
        let init = u32_to_disposition(initialization)?;

        let mut cards = self.registry.cards.write().unwrap();
        let (card, _) = cards.get_mut(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        card.reconnect(share, protos, init)
            .map_err(pcsc_error_to_u32)?;

        let mut r_buf = [0u8; 256];
        let mut a_buf = [0u8; 36];
        let new_proto = match card.status2(&mut r_buf, &mut a_buf) {
            Ok(status) => protocol_to_u32(status.protocol()),
            Err(_) => SCARD_PROTOCOL_T1,
        };
        Ok(new_proto)
    }

    fn transmit(
        &self,
        handle: &ScardHandle,
        _send_pci: &ScardIORequest,
        data: &[u8],
    ) -> Result<TransmitResult, u32> {
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let mut recv_buf = vec![0u8; SCARD_TRANSMIT_MAX];
        let resp_slice = card
            .transmit(data, &mut recv_buf)
            .map_err(pcsc_error_to_u32)?;

        Ok(TransmitResult {
            recv_pci: None,
            recv_buffer: resp_slice.to_vec(),
        })
    }

    fn control(
        &self,
        handle: &ScardHandle,
        control_code: u32,
        in_data: &[u8],
    ) -> Result<Vec<u8>, u32> {
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let mut out_buf = vec![0u8; 4096];
        let resp_slice = card
            .control(control_code, in_data, &mut out_buf)
            .map_err(pcsc_error_to_u32)?;

        Ok(resp_slice.to_vec())
    }

    fn status(&self, handle: &ScardHandle) -> Result<ScardStatus, u32> {
        let cards = self.registry.cards.read().unwrap();
        let (card, reader_name) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let mut r_buf = [0u8; 256];
        let mut a_buf = [0u8; 36];
        let status = card
            .status2(&mut r_buf, &mut a_buf)
            .map_err(pcsc_error_to_u32)?;

        Ok(ScardStatus {
            reader_names: vec![reader_name.clone()],
            state: pcsc_status_to_u32(status.status()),
            protocol: protocol_to_u32(status.protocol()),
            atr: status.atr().to_vec(),
        })
    }

    fn get_status_change(
        &self,
        ctx: &ScardContext,
        timeout: Duration,
        reader_states: &[ReaderStateIn],
    ) -> Result<(Vec<ReaderStateOut>, u32), u32> {
        let mut pcsc_states = Vec::new();
        for rs in reader_states {
            let cstr = std::ffi::CString::new(rs.reader_name.as_str())
                .map_err(|_| SCARD_E_INVALID_PARAMETER)?;
            let current_state = u32_to_state(rs.current_state);
            pcsc_states.push(pcsc::ReaderState::new(cstr, current_state));
        }

        let contexts = self.registry.contexts.read().unwrap();
        let pcsc_ctx = contexts.get(&ctx.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        // Mimic the FreeRDP reference channel: never block the IRP thread for more
        // than a small step (100ms). The caller re-polls on SCARD_E_TIMEOUT.
        let step = timeout.min(Duration::from_millis(100));
        let gsc_result = pcsc_ctx.get_status_change(step, &mut pcsc_states);

        let mut results = Vec::new();
        for (i, rs) in reader_states.iter().enumerate() {
            let out_state = &pcsc_states[i];
            // Read the raw SCARD_READERSTATE: the pcsc crate's event_state()
            // uses State::from_bits_truncate which drops the 0x0001_0000 bit
            // that Windows sets in reader states (e.g. for \\?PnP?\Notification).
            // The native FreeRDP channel passes these bits through verbatim, and
            // the remote SCardSvr expects them.
            let raw: &pcsc::ffi::SCARD_READERSTATE = unsafe { std::mem::transmute(out_state) };
            results.push(ReaderStateOut {
                reader_name: rs.reader_name.clone(),
                current_state: rs.current_state,
                event_state: raw.dwEventState,
                atr: raw.rgbAtr[..raw.cbAtr as usize].to_vec(),
            });
        }

        match gsc_result {
            Ok(()) => Ok((results, SCARD_S_SUCCESS)),
            Err(pcsc::Error::Timeout) => Ok((results, SCARD_E_TIMEOUT)),
            Err(e) => Err(pcsc_error_to_u32(e)),
        }
    }

    fn begin_transaction(&self, _handle: &ScardHandle) -> Result<(), u32> {
        // Transactions are serialized by the device thread and do not require locks at the local PC/SC layer.
        Ok(())
    }

    fn end_transaction(&self, _handle: &ScardHandle, _disposition: u32) -> Result<(), u32> {
        // Transactions are serialized by the device thread and do not require locks at the local PC/SC layer.
        Ok(())
    }

    fn get_attrib(&self, handle: &ScardHandle, attr_id: u32) -> Result<Vec<u8>, u32> {
        let attribute = u32_to_attribute(attr_id)?;
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let mut buf = vec![0u8; 1024];
        let slice = card
            .get_attribute(attribute, &mut buf)
            .map_err(pcsc_error_to_u32)?;
        Ok(slice.to_vec())
    }

    fn set_attrib(&self, handle: &ScardHandle, attr_id: u32, data: &[u8]) -> Result<(), u32> {
        let attribute = u32_to_attribute(attr_id)?;
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;
        card.set_attribute(attribute, data)
            .map_err(pcsc_error_to_u32)?;
        Ok(())
    }

    fn get_container_info(&self, handle: &ScardHandle, _container_index: u8) -> Result<Vec<u8>, u32> {
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        // GET DATA 7F49 (public key) — same APDU msclmd issues.
        let apdu = [
            0x00, 0xCB, 0x3F, 0xFF, 0x0A, 0x70, 0x08, 0x84, 0x01, 0x81, 0xA5, 0x03, 0x7F, 0x49, 0x80,
            0x00,
        ];
        let mut buf = vec![0u8; 1024];
        let resp = card.transmit(&apdu, &mut buf).map_err(pcsc_error_to_u32)?;
        let modulus = extract_modulus(resp).ok_or(SCARD_E_UNSUPPORTED_FEATURE)?;
        Ok(build_container_info(modulus))
    }

    fn is_available(&self) -> bool {
        pcsc::Context::establish(pcsc::Scope::System).is_ok()
    }
}
