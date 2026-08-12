// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

pub(crate) mod helpers;
pub(crate) mod types;

use std::time::Duration;

use pcsc::ffi::DWORD;
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
    fn establish_context(&self, _scope: DWORD) -> Result<ScardContext, u32> {
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
        share_mode: DWORD,
        preferred_protocols: DWORD,
    ) -> Result<ConnectResult, u32> {
        let contexts = self.registry.contexts.read().unwrap();
        let pcsc_ctx = contexts.get(&ctx.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let share = dword_to_share_mode(share_mode)?;
        let protos = dword_to_protocols(preferred_protocols);

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
        self.registry
            .ctx_cards
            .write()
            .unwrap()
            .insert(ctx.raw(), handle.raw());

        Ok(ConnectResult {
            handle,
            active_protocol: active_proto,
        })
    }

    fn disconnect(&self, handle: &ScardHandle, disposition: DWORD) -> Result<(), u32> {
        let disp = dword_to_disposition(disposition)?;
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
        share_mode: DWORD,
        preferred_protocols: DWORD,
        initialization: DWORD,
    ) -> Result<u32, u32> {
        let share = dword_to_share_mode(share_mode)?;
        let protos = dword_to_protocols(preferred_protocols);
        let init = dword_to_disposition(initialization)?;

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
        control_code: DWORD,
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
            let current_state = dword_to_state(rs.current_state.into());
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
                event_state: raw.dwEventState as u32,
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

    fn end_transaction(&self, _handle: &ScardHandle, _disposition: DWORD) -> Result<(), u32> {
        // Transactions are serialized by the device thread and do not require locks at the local PC/SC layer.
        Ok(())
    }

    fn get_attrib(&self, handle: &ScardHandle, attr_id: DWORD) -> Result<Vec<u8>, u32> {
        let attribute = dword_to_attribute(attr_id)?;
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let mut buf = vec![0u8; 1024];
        let slice = card
            .get_attribute(attribute, &mut buf)
            .map_err(pcsc_error_to_u32)?;
        Ok(slice.to_vec())
    }

    fn set_attrib(&self, handle: &ScardHandle, attr_id: DWORD, data: &[u8]) -> Result<(), u32> {
        let attribute = dword_to_attribute(attr_id)?;
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;
        card.set_attribute(attribute, data)
            .map_err(pcsc_error_to_u32)?;
        Ok(())
    }

    fn get_container_info(
        &self,
        _ctx: &ScardContext,
        _container_index: u8,
    ) -> Result<Vec<u8>, u32> {
        // ---------------------------------------------------------------------
        // FINDING (2026-08-05): intentionally returns Err so the addin reports a
        // READ_CACHE MISS and msclmd reads the container public key from the card
        // itself via TRANSMIT (GET DATA 7F49 + GET RESPONSE, chunked, ≤258 bytes
        // per response, NO PIN required) and WRITE_CACHEs it. Fully autonomous —
        // no OS-cache dependency. See docs/smartcard-connect-phase-discovery.md.
        //
        // CRITICAL byte-order detail (for the emulator): the card returns the RSA
        // modulus big-endian (`7F 49 ... 82 E2 17 BF ...`), but `ContainerInfo_00`
        // stores it LITTLE-ENDIAN (`4D 11 9C 3F ...` is the byte-reversed modulus).
        // `extract_modulus` yields the big-endian bytes; reverse them before feeding
        // `build_container_info`.
        //
        // Reference blob (what msclmd writes, 308 bytes): 12-byte CSP header
        // `00 00 01 00 05 00 00 00 00 00 00 00` + data length (292), a
        // CONTAINER_INFO (dwVersion/dwReserved/cbSigPublicKey = 0, cbKeyExPublicKey
        // = 276) and a PUBLICKEYBLOB (bType 0x06, aiKeyAlg CALG_RSA_KEYX
        // 0x0000A400, "RSA1", 2048 bits, exponent 0x010001, little-endian modulus).
        // `build_container_info` currently does NOT reverse the modulus and uses a
        // different CSP header byte (0x02 vs 0x05) — fix both when implementing.
        // ---------------------------------------------------------------------
        Err(SCARD_E_UNSUPPORTED_FEATURE)
    }

    fn get_certificate(&self, ctx: &ScardContext) -> Result<Vec<u8>, u32> {
        let handle_id = self
            .registry
            .ctx_cards
            .read()
            .unwrap()
            .get(&ctx.raw())
            .copied()
            .ok_or(SCARD_E_INVALID_HANDLE)?;
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle_id).ok_or(SCARD_E_INVALID_HANDLE)?;

        // GET DATA DF24 (certificate file) — same APDU msclmd issues.
        let apdu = [0x00, 0xCB, 0xA0, 0x10, 0x04, 0x5C, 0x02, 0xDF, 0x24, 0x00];
        let mut buf = vec![0u8; 4096];
        let mut full = match card.transmit(&apdu, &mut buf) {
            Ok(r) => r.to_vec(),
            Err(e) => {
                log::debug!(
                    "smartcard native: get_certificate DF24 transmit error: {:?}",
                    e
                );
                return Err(pcsc_error_to_u32(e));
            }
        };

        // GET RESPONSE chaining (61 XX)
        loop {
            let len = full.len();
            if len < 2 {
                break;
            }
            let (sw1, sw2) = (full[len - 2], full[len - 1]);
            if sw1 != 0x61 {
                break;
            }
            let remaining = sw2 as usize;
            full.truncate(len - 2);
            let get_resp = [
                0x00,
                0xC0,
                0x00,
                0x00,
                if remaining == 0 { 0x00 } else { sw2 },
            ];
            match card.transmit(&get_resp, &mut buf) {
                Ok(r) => full.extend_from_slice(r),
                Err(e) => {
                    log::debug!(
                        "smartcard native: get_certificate GET RESPONSE error: {:?}",
                        e
                    );
                    return Err(pcsc_error_to_u32(e));
                }
            }
        }

        let content = match strip_do(&full) {
            Some(c) => c.to_vec(),
            None => {
                log::debug!("smartcard native: get_certificate could not parse DF24 DO");
                return Err(SCARD_E_UNSUPPORTED_FEATURE);
            }
        };
        log::debug!(
            "smartcard native: get_certificate content {} bytes (resp {} bytes)",
            content.len(),
            full.len()
        );
        Ok(build_general_file_value(&content))
    }

    fn is_available(&self) -> bool {
        pcsc::Context::establish(pcsc::Scope::System).is_ok()
    }
}
