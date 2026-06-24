// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! GIDS engine — processes ISO 7816 APDUs for the GIDS protocol.

use num_bigint::BigUint;
use rsa::RsaPrivateKey;
use rsa::pkcs1::EncodeRsaPrivateKey;

use super::consts::*;
use super::helpers::*;
use super::types::*;

/// The core GIDS smartcard engine.
pub struct GidsEngine {
    /// Certificate DER bytes (stored for future VFS reconstruction)
    #[allow(dead_code)]
    pub cert_der: Vec<u8>,
    pub pin: String,
    pub vfs: VirtualFs,
    pub session: SessionState,
    pub n: BigUint,
    pub d: BigUint,
    pub key_size: usize,
}

impl std::fmt::Debug for GidsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GidsEngine")
            .field("key_size", &self.key_size)
            .field("pin_verified", &self.session.pin_verified)
            .finish()
    }
}

impl GidsEngine {
    pub fn new(cert_der: Vec<u8>, private_key: RsaPrivateKey, pin: String) -> Self {
        let pkcs1_der = private_key
            .to_pkcs1_der()
            .expect("failed to serialize RSA key to PKCS#1 DER");
        let (n, d) = parse_rsa_pkcs1_components(pkcs1_der.as_bytes())
            .expect("failed to parse RSA PKCS#1 components");
        let key_size = (n.bits() as usize).div_ceil(8);
        let vfs = VirtualFs::new(&cert_der, &n, &d, key_size);
        GidsEngine {
            cert_der,
            pin,
            vfs,
            session: SessionState::new(),
            n,
            d,
            key_size,
        }
    }

    pub fn process_apdu(&mut self, apdu: &[u8]) -> Vec<u8> {
        let Some(header) = parse_apdu_header(apdu) else {
            return make_status(SW_WRONG_LC);
        };
        let (data, le) = extract_apdu_data(apdu);

        if header.ins == 0x2A && (header.cla & 0x10) != 0 {
            self.session.command_buffer.extend_from_slice(data);
            return make_status(SW_MORE_DATA);
        }

        match header.ins {
            0xA4 => self.select(header.p1, header.p2, data),
            0xCB | 0xCA => self.get_data(header.p1, header.p2, data),
            0xC0 => self.get_response(le),
            0x22 => self.mse_set(header.p1, header.p2, data),
            0x2A => self.pso(header.p1, header.p2, data),
            0x20 => self.verify(header.p1, header.p2, data),
            _ => make_status(SW_COMMAND_NOT_ALLOWED),
        }
    }

    fn select(&mut self, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
        match p1 {
            0x04 => {
                if data.len() > 16 {
                    return make_status(SW_WRONG_LC);
                }
                // Accept the full 11-byte GIDS AID or the 9-byte prefix
                let is_gids = data == GIDS_AID || (data.len() == 9 && &data[..9] == &GIDS_AID[..9]);
                if !is_gids {
                    return make_status(SW_FILE_NOT_FOUND);
                }
                // Accept any P2 value (0x00=FCI, 0x04=FCP, 0x0C=no FCI, etc.)
                self.session.current_df = ISO_FID_MF;
                match p2 & 0x03 {
                    0x00 => make_response(&GIDS_FCI, SW_SUCCESS),
                    0x04 => make_response(&GIDS_FCP, SW_SUCCESS),
                    _ => make_status(SW_SUCCESS),
                }
            }
            0x00 => {
                // Select by FID — accept any P2
                if data.len() != 2 {
                    return make_status(SW_WRONG_LC);
                }
                let fid = ((data[0] as u16) << 8) | (data[1] as u16);
                if fid == EFID_CURRENT_DF && self.session.current_df != 0 {
                    make_status(SW_SUCCESS)
                } else if fid == ISO_FID_MF {
                    self.session.current_df = ISO_FID_MF;
                    make_status(SW_SUCCESS)
                } else {
                    log::debug!(
                        "smartcard: SELECT FID 0x{:04X} failed (current_df=0x{:04X})",
                        fid,
                        self.session.current_df
                    );
                    make_status(SW_FILE_NOT_FOUND)
                }
            }
            _ => make_status(SW_INVALID_P1P2),
        }
    }

    fn get_data(&mut self, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
        let file_id = ((p1 as u16) << 8) | (p2 as u16);

        // Handle Le-only (no data, just asking for response) — e.g. "00 CA 7F 68 00"
        if data.is_empty() {
            // Unknown tag — return the VFS data for probing requests
            return make_status(SW_REF_DATA_NOT_FOUND);
        }

        // Handle tag list with empty list: "5C 00" — return all DOs from the EF
        if data.len() == 2 && data[0] == 0x5C && data[1] == 0x00 {
            let search_ef = if file_id == EFID_CURRENT_DF {
                self.session.current_df
            } else {
                file_id
            };
            // Return the EF content, falling back to Master EF for unknown IDs
            if let Some(ef_data) = self.vfs.get_ef_data(search_ef) {
                let ef_data = ef_data.to_vec();
                self.set_response_buffer(&ef_data);
                self.emit_response_chunk(None)
            } else if let Some(ef_data) = self.vfs.get_ef_data(EFID_MASTER) {
                // Fallback: return Master EF (filesystem table + key map)
                let ef_data = ef_data.to_vec();
                self.set_response_buffer(&ef_data);
                self.emit_response_chunk(None)
            } else {
                make_status(SW_REF_DATA_NOT_FOUND)
            }
        } else if data.len() == 4 {
            if data[0] != 0x5C || data[1] != 0x02 {
                return make_status(SW_INVALID_COMMAND_DATA);
            }
            let do_id = ((data[2] as u16) << 8) | (data[3] as u16);
            let search_ef = if file_id == EFID_CURRENT_DF {
                self.session.current_df
            } else {
                file_id
            };
            let result = if search_ef != 0 {
                self.vfs.read_do(search_ef, do_id).map(|d| d.to_vec())
            } else {
                None
            }
            .or_else(|| {
                [EFID_MASTER, EFID_COMMON, EFID_CARDID]
                    .iter()
                    .find_map(|ef| self.vfs.read_do(*ef, do_id).map(|d| d.to_vec()))
            });

            if let Some(do_data) = result {
                self.set_response_buffer(&do_data);
                self.emit_response_chunk(None)
            } else {
                make_status(SW_REF_DATA_NOT_FOUND)
            }
        } else if data.len() == 10 || data.len() == 9 {
            if file_id != EFID_CURRENT_DF {
                return make_status(SW_INVALID_P1P2);
            }
            let pub_key = self.get_public_key_response();
            self.set_response_buffer(&pub_key);
            self.emit_response_chunk(None)
        } else {
            make_status(SW_INVALID_COMMAND_DATA)
        }
    }

    fn get_response(&mut self, le: Option<u8>) -> Vec<u8> {
        if self.session.response_buffer.is_empty() {
            return make_status(SW_WRONG_LC);
        }
        self.emit_response_chunk(le)
    }

    fn mse_set(&mut self, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
        if p1 != 0x41 || (p2 != CRT_SIGN && p2 != CRT_CONF) {
            return make_status(SW_INVALID_P1P2);
        }
        if data.len() != 6
            || data[0] != 0x80
            || data[1] != 0x01
            || data[3] != 0x84
            || data[4] != 0x01
        {
            return make_status(SW_INVALID_COMMAND_DATA);
        }
        self.session.current_se = Some(SecurityEnvironment {
            crt: p2,
            algo_id: data[2],
            key_ref: data[5],
        });
        make_status(SW_SUCCESS)
    }

    fn pso(&mut self, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
        let full_data = if !self.session.command_buffer.is_empty() {
            let mut combined = std::mem::take(&mut self.session.command_buffer);
            combined.extend_from_slice(data);
            combined
        } else {
            data.to_vec()
        };

        let se = match self.session.current_se {
            Some(se) => se,
            None => return make_status(SW_SECURITY_NOT_SATISFIED),
        };
        if !self.session.pin_verified || se.key_ref != DEFAULT_KEY_REF {
            return make_status(SW_SECURITY_NOT_SATISFIED);
        }

        match se.crt {
            CRT_SIGN => {
                if p1 != 0x9E || p2 != 0x9A {
                    make_status(SW_INVALID_P1P2)
                } else {
                    self.perform_signature(&full_data)
                }
            }
            CRT_CONF => {
                if !((p1 == 0x86 && p2 == 0x80) || (p1 == 0x80 && p2 == 0x86)) {
                    make_status(SW_INVALID_P1P2)
                } else {
                    self.perform_decrypt(&full_data, se.algo_id)
                }
            }
            _ => make_status(SW_INVALID_P1P2),
        }
    }

    fn verify(&mut self, _p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
        match p2 {
            0x82 => {
                self.session.pin_verified = false;
                make_status(SW_SUCCESS)
            }
            0x80 => {
                if self.session.pin_retries == 0 {
                    return make_status(SW_AUTH_METHOD_BLOCKED);
                }
                if data.len() > MAX_PIN_SIZE {
                    return make_status(SW_WRONG_LC);
                }
                let input_pin = String::from_utf8_lossy(data);
                if input_pin == self.pin {
                    self.session.pin_verified = true;
                    self.session.pin_retries = DEFAULT_PIN_RETRIES;
                    make_status(SW_SUCCESS)
                } else {
                    self.session.pin_verified = false;
                    self.session.pin_retries -= 1;
                    make_status(SW_VERIFY_FAILED | self.session.pin_retries as u16)
                }
            }
            _ => make_status(SW_INVALID_P1P2),
        }
    }

    fn perform_signature(&mut self, digest_info: &[u8]) -> Vec<u8> {
        match self.rsa_pkcs1_sign(digest_info) {
            Ok(sig) => {
                self.set_response_buffer(&sig);
                self.emit_response_chunk(None)
            }
            Err(e) => {
                log::error!("smartcard: RSA sign failed: {}", e);
                make_status(SW_INVALID_COMMAND_DATA)
            }
        }
    }

    fn perform_decrypt(&mut self, ciphertext: &[u8], algo_id: u8) -> Vec<u8> {
        let result = if algo_id & ALGID_PAD_OAEP != 0 {
            self.rsa_decrypt_oaep(ciphertext)
        } else if algo_id & ALGID_PAD_PKCS1 != 0 {
            self.rsa_decrypt_pkcs1(ciphertext)
        } else {
            self.rsa_decrypt_raw(ciphertext)
        };
        match result {
            Ok(pt) => {
                self.set_response_buffer(&pt);
                self.emit_response_chunk(None)
            }
            Err(e) => {
                log::error!("smartcard: RSA decrypt failed: {}", e);
                make_status(SW_INVALID_COMMAND_DATA)
            }
        }
    }

    fn rsa_raw(&self, value: &[u8]) -> Vec<u8> {
        let v = BigUint::from_bytes_be(value);
        let result = v.modpow(&self.d, &self.n);
        let mut bytes = result.to_bytes_be();
        if bytes.len() < self.key_size {
            let mut padded = vec![0u8; self.key_size - bytes.len()];
            padded.extend_from_slice(&bytes);
            bytes = padded;
        }
        bytes
    }

    fn rsa_pkcs1_sign(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        if data.len() + 11 > self.key_size {
            return Err("Data too large".to_string());
        }
        let mut em = vec![0u8; self.key_size];
        em[0] = 0x00;
        em[1] = 0x01;
        let ps_len = self.key_size - data.len() - 3;
        for i in 0..ps_len {
            em[2 + i] = 0xFF;
        }
        em[2 + ps_len] = 0x00;
        em[3 + ps_len..].copy_from_slice(data);
        Ok(self.rsa_raw(&em))
    }

    fn rsa_decrypt_pkcs1(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        let em = self.rsa_raw(ciphertext);
        if em.len() < 11 || em[0] != 0x00 || em[1] != 0x02 {
            return Err("Invalid padding".to_string());
        }
        let sep = em[2..].iter().position(|&b| b == 0x00);
        match sep {
            Some(idx) if idx >= 8 => Ok(em[3 + idx..].to_vec()),
            _ => Err("Invalid padding".to_string()),
        }
    }

    fn rsa_decrypt_oaep(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        let em = self.rsa_raw(ciphertext);
        if em.len() < 42 || em[0] != 0x00 {
            return Err("Invalid OAEP".to_string());
        }
        let h_len = 20;
        let seed_mask = mgf1(&em[1 + h_len..], h_len);
        let seed: Vec<u8> = em[1..1 + h_len]
            .iter()
            .zip(seed_mask.iter())
            .map(|(a, b)| a ^ b)
            .collect();
        let db_mask = mgf1(&seed, em.len() - 1 - h_len);
        let db: Vec<u8> = em[1 + h_len..]
            .iter()
            .zip(db_mask.iter())
            .map(|(a, b)| a ^ b)
            .collect();
        let sep = db[h_len..].iter().position(|&b| b == 0x01);
        match sep {
            Some(idx) => Ok(db[h_len + idx + 1..].to_vec()),
            None => Err("Invalid OAEP".to_string()),
        }
    }

    fn rsa_decrypt_raw(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        Ok(self.rsa_raw(ciphertext))
    }

    fn get_public_key_response(&self) -> Vec<u8> {
        let n_bytes = self.n.to_bytes_be();
        let e = BigUint::from(65537u32);
        let e_bytes = e.to_bytes_be();
        let mut inner = Vec::new();
        tlv_write(&mut inner, 0x81, &n_bytes);
        tlv_write(&mut inner, 0x82, &e_bytes);
        let mut outer = Vec::new();
        tlv_write(&mut outer, 0x7F49, &inner);
        outer
    }

    fn set_response_buffer(&mut self, data: &[u8]) {
        self.session.response_buffer = data.to_vec();
    }

    fn emit_response_chunk(&mut self, le: Option<u8>) -> Vec<u8> {
        let max_chunk = le.unwrap_or(0) as usize;
        let max_chunk = if max_chunk == 0 || max_chunk > 256 {
            256
        } else {
            max_chunk
        };
        let take = self.session.response_buffer.len().min(max_chunk);
        let chunk: Vec<u8> = self.session.response_buffer.drain(..take).collect();
        let remaining = self.session.response_buffer.len();
        if remaining > 0 {
            make_response(&chunk, SW_MORE_DATA | remaining.min(0xFF) as u16)
        } else {
            make_response(&chunk, SW_SUCCESS)
        }
    }
}
