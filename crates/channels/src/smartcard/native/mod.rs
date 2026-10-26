// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com
//          Janier Rodríguez, jrodriguez at virtualcable dot es

pub(crate) mod helpers;
pub(crate) mod raw;
pub(crate) mod types;

use std::time::Duration;

use pcsc::ffi::{self, DWORD, SCARD_READERSTATE};
use rdp::integrations::smartcard::*;

use super::SmartcardBackend;
use helpers::*;
use raw::{RawContext, pci_for};
use types::NativeRegistry;

const MAX_CERTIFICATE_SIZE: usize = 1024 * 1024;
const MAX_GET_RESPONSE_ROUNDS: usize = 4096;

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
        let raw_ctx = RawContext::establish()?;
        let sc_ctx = ScardContext::new();
        let mut contexts = self.registry.contexts.write().unwrap();
        contexts.insert(sc_ctx.raw(), raw_ctx);
        Ok(sc_ctx)
    }

    fn release_context(&self, ctx: &ScardContext) -> Result<(), u32> {
        let card_ids = self
            .registry
            .ctx_cards
            .write()
            .unwrap()
            .take_cards(ctx.raw());

        let mut cards = self.registry.cards.write().unwrap();
        for card_id in card_ids {
            if let Some((card, _)) = cards.remove(&card_id) {
                let _ = card.disconnect(ffi::SCARD_LEAVE_CARD);
            }
        }

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
        let raw_ctx = contexts.get(&ctx.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        match raw_ctx.list_readers() {
            Err(SCARD_E_NO_READERS_AVAILABLE) => Ok(Vec::new()),
            other => other,
        }
    }

    fn connect(
        &self,
        ctx: &ScardContext,
        reader: &str,
        share_mode: DWORD,
        preferred_protocols: DWORD,
    ) -> Result<ConnectResult, u32> {
        let contexts = self.registry.contexts.read().unwrap();
        let raw_ctx = contexts.get(&ctx.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let share = dword_to_share_mode(share_mode)?;

        let reader_c = std::ffi::CString::new(reader).map_err(|_| SCARD_E_INVALID_PARAMETER)?;
        let (card, active_proto) = raw_ctx.connect(&reader_c, share, preferred_protocols)?;
        let active_proto = active_protocol_to_u32(active_proto);

        let handle = ScardHandle::new(active_proto);
        let mut cards = self.registry.cards.write().unwrap();
        cards.insert(handle.raw(), (card, reader.to_string()));
        self.registry
            .ctx_cards
            .write()
            .unwrap()
            .add(ctx.raw(), handle.raw());

        Ok(ConnectResult {
            handle,
            active_protocol: active_proto,
        })
    }

    fn disconnect(&self, handle: &ScardHandle, disposition: DWORD) -> Result<(), u32> {
        let disp = dword_to_disposition(disposition)?;
        let mut cards = self.registry.cards.write().unwrap();
        if let Some((card, _)) = cards.remove(&handle.raw()) {
            self.registry
                .ctx_cards
                .write()
                .unwrap()
                .remove_card(handle.raw());
            card.disconnect(disp)
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
        let init = dword_to_disposition(initialization)?;

        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let new_proto = card.reconnect(share, preferred_protocols, init)?;
        Ok(active_protocol_to_u32(new_proto))
    }

    fn transmit(
        &self,
        handle: &ScardHandle,
        send_pci: &ScardIORequest,
        data: &[u8],
    ) -> Result<TransmitResult, u32> {
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let pci = pci_for(send_pci.protocol)
            .or_else(|| pci_for(handle.active_protocol()))
            .ok_or(SCARD_E_INVALID_PARAMETER)?;
        let mut recv_buf = vec![0u8; SCARD_TRANSMIT_MAX];
        let len = card.transmit(pci, data, &mut recv_buf)?;
        recv_buf.truncate(len);

        let recv_pci = card.status().ok().map(|status| ScardIORequest {
            protocol: active_protocol_to_u32(status.protocol),
            extra_bytes: Vec::new(),
        });

        Ok(TransmitResult {
            recv_pci,
            recv_buffer: recv_buf,
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
        let len = card.control(control_code, in_data, &mut out_buf)?;
        out_buf.truncate(len);
        Ok(out_buf)
    }

    fn status(&self, handle: &ScardHandle) -> Result<ScardStatus, u32> {
        let cards = self.registry.cards.read().unwrap();
        let (card, reader_name) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        let status = card.status()?;

        let result = ScardStatus {
            reader_names: vec![reader_name.clone()],
            state: pcsc_status_to_u32(pcsc::Status::from_bits_truncate(status.state)),
            protocol: active_protocol_to_u32(status.protocol),
            atr: status.atr,
        };
        log::trace!(
            "smartcard native: status 0x{:X}: pcsc=0x{:X} -> state={} protocol=0x{:X}",
            handle.raw(),
            status.state,
            result.state,
            result.protocol
        );
        Ok(result)
    }

    fn get_status_change(
        &self,
        ctx: &ScardContext,
        timeout: Duration,
        reader_states: &[ReaderStateIn],
    ) -> Result<(Vec<ReaderStateOut>, u32), u32> {
        let mut names = Vec::with_capacity(reader_states.len());
        for rs in reader_states {
            names.push(
                std::ffi::CString::new(rs.reader_name.as_str())
                    .map_err(|_| SCARD_E_INVALID_PARAMETER)?,
            );
        }
        // State bits pass through verbatim both ways: Windows sets bits such as
        // 0x0001_0000 (e.g. for \\?PnP?\Notification) that the remote SCardSvr
        // expects back, and the FreeRDP channel does not filter them either.
        let mut raw_states: Vec<SCARD_READERSTATE> = reader_states
            .iter()
            .zip(&names)
            .map(|(rs, name)| SCARD_READERSTATE {
                szReader: name.as_ptr(),
                pvUserData: std::ptr::null_mut(),
                dwCurrentState: rs.current_state as DWORD,
                dwEventState: 0,
                cbAtr: 0,
                rgbAtr: [0u8; ffi::ATR_BUFFER_SIZE],
            })
            .collect();

        let contexts = self.registry.contexts.read().unwrap();
        let raw_ctx = contexts.get(&ctx.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        // Mimic the FreeRDP reference channel: never block the IRP thread for more
        // than a small step (100ms). The caller re-polls on SCARD_E_TIMEOUT.
        let step = timeout.min(Duration::from_millis(100));
        let gsc_result = raw_ctx.get_status_change(step, &mut raw_states);

        let results = reader_states
            .iter()
            .zip(&raw_states)
            .map(|(rs, raw)| {
                let atr_len = (raw.cbAtr as usize).min(raw.rgbAtr.len());
                ReaderStateOut {
                    reader_name: rs.reader_name.clone(),
                    current_state: rs.current_state,
                    event_state: raw.dwEventState as u32,
                    atr: raw.rgbAtr[..atr_len].to_vec(),
                }
            })
            .collect();

        match gsc_result {
            Ok(()) => Ok((results, SCARD_S_SUCCESS)),
            Err(SCARD_E_TIMEOUT) => Ok((results, SCARD_E_TIMEOUT)),
            Err(code) => Err(code),
        }
    }

    fn cancel(&self, ctx: &ScardContext) -> Result<(), u32> {
        let contexts = self.registry.contexts.read().unwrap();
        let raw_ctx = contexts.get(&ctx.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;
        raw_ctx.cancel()
    }

    fn begin_transaction(&self, handle: &ScardHandle) -> Result<(), u32> {
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;
        card.begin_transaction()
    }

    fn end_transaction(&self, handle: &ScardHandle, disposition: DWORD) -> Result<(), u32> {
        let disp = dword_to_disposition(disposition)?;
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;
        card.end_transaction(disp)
    }

    fn get_attrib(&self, handle: &ScardHandle, attr_id: DWORD) -> Result<Vec<u8>, u32> {
        let cards = self.registry.cards.read().unwrap();
        let (card, reader_name) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;

        if let Some(value) = system_attribute(attr_id, reader_name) {
            log::trace!(
                "smartcard native: get_attrib 0x{:X}: system attribute ({} bytes)",
                attr_id,
                value.len()
            );
            return Ok(value);
        }
        if dword_to_attribute(attr_id).is_err() {
            log::trace!(
                "smartcard native: get_attrib 0x{:X}: not supported",
                attr_id
            );
            return Err(ERROR_NOT_SUPPORTED);
        }

        let mut buf = vec![0u8; 1024];
        let result = card.get_attrib(attr_id, &mut buf).map(|len| {
            buf.truncate(len);
            buf
        });
        log::trace!(
            "smartcard native: get_attrib 0x{:X}: {:?}",
            attr_id,
            result.as_ref().map(Vec::len)
        );
        result
    }

    fn set_attrib(&self, handle: &ScardHandle, attr_id: DWORD, data: &[u8]) -> Result<(), u32> {
        dword_to_attribute(attr_id)?;
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle.raw()).ok_or(SCARD_E_INVALID_HANDLE)?;
        card.set_attrib(attr_id, data)
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
            .first_card(ctx.raw())
            .ok_or(SCARD_E_INVALID_HANDLE)?;
        let cards = self.registry.cards.read().unwrap();
        let (card, _) = cards.get(&handle_id).ok_or(SCARD_E_INVALID_HANDLE)?;
        let pci = pci_for(active_protocol_to_u32(card.status()?.protocol))
            .ok_or(SCARD_E_INVALID_PARAMETER)?;

        // GET DATA DF24 (certificate file) — same APDU msclmd issues.
        let apdu = [0x00, 0xCB, 0xA0, 0x10, 0x04, 0x5C, 0x02, 0xDF, 0x24, 0x00];
        let mut buf = vec![0u8; 4096];
        let mut full = match card.transmit(pci, &apdu, &mut buf) {
            Ok(len) => buf[..len].to_vec(),
            Err(e) => {
                log::debug!(
                    "smartcard native: get_certificate DF24 transmit error: {:#X}",
                    e
                );
                return Err(e);
            }
        };

        // GET RESPONSE chaining (61 XX)
        for _ in 0..MAX_GET_RESPONSE_ROUNDS {
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
            match card.transmit(pci, &get_resp, &mut buf) {
                Ok(len) => {
                    if full.len().saturating_add(len) > MAX_CERTIFICATE_SIZE {
                        return Err(SCARD_E_INSUFFICIENT_BUFFER);
                    }
                    full.extend_from_slice(&buf[..len]);
                }
                Err(e) => {
                    log::debug!(
                        "smartcard native: get_certificate GET RESPONSE error: {:#X}",
                        e
                    );
                    return Err(e);
                }
            }
        }

        if full.len() >= 2 && full[full.len() - 2] == 0x61 {
            return Err(SCARD_E_UNSUPPORTED_FEATURE);
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
        RawContext::establish().is_ok()
    }
}
