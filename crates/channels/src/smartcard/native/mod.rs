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
        self.registry.ctx_cards.write().unwrap().insert(ctx.raw(), handle.raw());

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

    fn get_container_info(&self, ctx: &ScardContext, _container_index: u8) -> Result<Vec<u8>, u32> {
        // ---------------------------------------------------------------------
        // FINDING (why the container pubkey must come from the OS cache):
        //
        // 1) msclmd's receive buffer over the RDPSC wire is FIXED at 258 bytes and
        //    it does NOT retry when a response is larger. The container public-key
        //    DO served for the *keyexchange* key is a single >258-byte response,
        //    so it can never reach msclmd through the redirect.
        //
        // 2) The card requires PIN authorization to SELECT the keyexchange key
        //    (MSE SET `00 22 41 B8 ... 87` returns SW `69 82` = Security status not
        //    satisfied). msclmd WOULD prompt for the PIN during container access
        //    (normal flow), but since (1) blocks the read anyway, serving the key
        //    from the OS cache avoids the PIN requirement entirely.
        //
        // The native FreeRDP channel "works" because it uses the Windows OS card
        // cache (SCardReadCacheW): `Cached_ContainerInfo_00` holds the keyexchange
        // public key (modulus 0x4D11...) from a previous (PIN-authorized) session.
        //
        // IMPORTANT: GET DATA `7F 49` WITHOUT selecting the keyexchange key returns
        // the *certificate* (signature) key (modulus 0x82E2...). Using that for
        // ContainerInfo_00 makes certutil's "prueba de coincidencia" FAIL.
        //
        // TODO (self-contained / emulated card): for a card we control, expose the
        // keyexchange key read without the PIN gate, or implement the PIN flow so
        // the container key can be read directly and the OS-cache dependency
        // removed.
        // ---------------------------------------------------------------------

        // Locate the connected card for this context.
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

        // Read the card identifier (GET DATA DF20 at EF_CARDID A012) — required as
        // the key for SCardReadCacheW. This is the same value msclmd reads during
        // the discovery phase.
        let mut cardid = [0u8; 16];
        {
            let apdu = [0x00, 0xCB, 0xA0, 0x12, 0x04, 0x5C, 0x02, 0xDF, 0x20, 0x00];
            let mut buf = vec![0u8; 64];
            match card.transmit(&apdu, &mut buf) {
                Ok(r) if r.len() >= 3 + 16 && r[2] == 0x10 => {
                    cardid.copy_from_slice(&r[3..3 + 16]);
                }
                _ => {
                    log::debug!("smartcard native: get_container_info could not read cardid");
                    return Err(SCARD_E_UNSUPPORTED_FEATURE);
                }
            }
        }

        // DIAGNOSTIC (light try): always attempt the autonomous keyexchange-key
        // read (MSE SET 0x87 + GET DATA 7F49) and log its modulus, so we can check
        // whether it yields 0x4D11 (keyexchange) or 0x82E2 (cert/signature) without
        // breaking the working OS-cache path.
        {
            let mut dbuf = vec![0u8; 1024];
            let mse = [0x00, 0x22, 0x41, 0xB8, 0x06, 0x80, 0x01, 0x87, 0x84, 0x01, 0x81];
            let mse_r = card.transmit(&mse, &mut dbuf).map(|r| r.to_vec());
            let pk_r = card
                .transmit(&[0x00, 0xCB, 0x3F, 0xFF, 0x0A, 0x70, 0x08, 0x84, 0x01, 0x81, 0xA5, 0x03, 0x7F, 0x49, 0x80, 0x00], &mut dbuf)
                .map(|r| r.to_vec());
            match (mse_r, pk_r) {
                (Ok(mse), Ok(pk)) => {
                    let head = extract_modulus(&pk)
                        .map(|m| m.iter().take(8).map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "));
                    log::debug!(
                        "smartcard native: [DIAG] MSE SET 0x87 -> [{}] 7F49 modulus head={:?}",
                        mse.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" "),
                        head
                    );
                }
                (mse, pk) => log::debug!("smartcard native: [DIAG] err mse={:?} pk={:?}", mse.err(), pk.err()),
            }
        }

        // Try the Windows OS cache first (what the native channel uses).
        let mut h_context: freerdp_sys::SCARDCONTEXT = 0;
        const SCARD_SCOPE_SYSTEM: u32 = 2;
        let est = unsafe {
            freerdp_sys::SCardEstablishContext(
                SCARD_SCOPE_SYSTEM,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut h_context,
            )
        };
        if est == 0 {
            let mut buf = vec![0u8; 1024];
            let mut cb = buf.len() as u32;
            let name = "Cached_ContainerInfo_00\0";
            let name_w: Vec<u16> = name.encode_utf16().collect();
            let ret = unsafe {
                freerdp_sys::SCardReadCacheW(
                    h_context,
                    cardid.as_mut_ptr() as *mut _,
                    1,
                    name_w.as_ptr() as *mut _,
                    buf.as_mut_ptr(),
                    &mut cb,
                )
            };
            unsafe {
                freerdp_sys::SCardReleaseContext(h_context);
            }
            if ret == 0 {
                buf.truncate(cb as usize);
                log::debug!("smartcard native: get_container_info OS-cache HIT ({} bytes)", buf.len());
                return Ok(buf);
            }
            log::debug!("smartcard native: get_container_info OS-cache miss (rc=0x{:X})", ret);
        }

        // Fallback: generate from the card. First try selecting the KEYEXCHANGE
        // key via MSE SET (the native does this: `00 22 41 B8 ... 87`), then read
        // the public key. If MSE SET selects the keyexchange key, GET DATA 7F49
        // should return its modulus (0x4D11...) instead of the certificate
        // (signature) key (0x82E2...). EXPERIMENTAL — this is the autonomous path.
        let mut buf = vec![0u8; 1024];
        let mse_set = [0x00, 0x22, 0x41, 0xB8, 0x06, 0x80, 0x01, 0x87, 0x84, 0x01, 0x81];
        match card.transmit(&mse_set, &mut buf) {
            Ok(r) => log::debug!(
                "smartcard native: MSE SET key=0x87 -> [{}]",
                r.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ")
            ),
            Err(e) => log::debug!("smartcard native: MSE SET error: {:?}", e),
        }
        let mut full = match card.transmit(&[0x00, 0xCB, 0x3F, 0xFF, 0x0A, 0x70, 0x08, 0x84, 0x01, 0x81, 0xA5, 0x03, 0x7F, 0x49, 0x80, 0x00], &mut buf) {
            Ok(r) => r.to_vec(),
            Err(e) => {
                log::debug!("smartcard native: get_container_info 7F49 transmit error: {:?}", e);
                return Err(pcsc_error_to_u32(e));
            }
        };

        // GET RESPONSE chaining (61 XX) — the pubkey DO exceeds the card's Le-limited
        // first chunk, so the rest must be fetched explicitly.
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
            let get_resp = [0x00, 0xC0, 0x00, 0x00, if remaining == 0 { 0x00 } else { sw2 }];
            match card.transmit(&get_resp, &mut buf) {
                Ok(r) => full.extend_from_slice(r),
                Err(e) => {
                    log::debug!("smartcard native: get_container_info GET RESPONSE error: {:?}", e);
                    return Err(pcsc_error_to_u32(e));
                }
            }
        }

        let tail: String = full
            .iter()
            .rev()
            .take(4)
            .rev()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        log::debug!(
            "smartcard native: get_container_info 7F49 full response ({} bytes) tail=[{}]",
            full.len(),
            tail
        );
        let modulus = match extract_modulus(&full) {
            Some(m) => m.to_vec(),
            None => {
                log::debug!("smartcard native: get_container_info modulus not found in response");
                return Err(SCARD_E_UNSUPPORTED_FEATURE);
            }
        };
        let mod_hex: String = modulus.iter().take(8).map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
        log::debug!("smartcard native: get_container_info modulus head=[{}] len={}", mod_hex, modulus.len());
        Ok(build_container_info(&modulus))
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
                log::debug!("smartcard native: get_certificate DF24 transmit error: {:?}", e);
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
            let get_resp = [0x00, 0xC0, 0x00, 0x00, if remaining == 0 { 0x00 } else { sw2 }];
            match card.transmit(&get_resp, &mut buf) {
                Ok(r) => full.extend_from_slice(r),
                Err(e) => {
                    log::debug!("smartcard native: get_certificate GET RESPONSE error: {:?}", e);
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
