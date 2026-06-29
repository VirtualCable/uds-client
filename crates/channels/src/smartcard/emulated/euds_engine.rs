// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! eUDS Engine — processes ISO 7816 APDUs for the eUDS custom protocol.

use num_bigint::BigUint;
use rsa::RsaPrivateKey;
use rsa::pkcs1::EncodeRsaPrivateKey;

use super::consts::*;
use super::euds_types::{PinMode, SessionState};
use super::helpers::*;

pub struct EudsEngine {
    pub cert_der: Vec<u8>,
    pub pin_mode: PinMode,
    pub pin: String,
    pub n: BigUint,
    pub d: BigUint,
    pub e: BigUint,
    pub key_size: usize,
    pub session: SessionState,
}

impl std::fmt::Debug for EudsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EudsEngine")
            .field("key_size", &self.key_size)
            .field("pin_mode", &self.pin_mode)
            .field("pin_verified", &self.session.pin_verified)
            .finish()
    }
}

impl EudsEngine {
    pub fn new(cert_der: Vec<u8>, private_key: RsaPrivateKey, pin: String, pin_mode: PinMode) -> Self {
        let pkcs1_der = private_key
            .to_pkcs1_der()
            .expect("failed to serialize RSA key to PKCS#1 DER");
        let (n, e, d) = parse_rsa_pkcs1_components(pkcs1_der.as_bytes())
            .expect("failed to parse RSA PKCS#1 components (n, e, d)");
        let key_size = (n.bits() as usize).div_ceil(8);
        EudsEngine {
            cert_der,
            pin_mode,
            pin,
            n,
            d,
            e,
            key_size,
            session: SessionState::default(),
        }
    }

    pub fn process_apdu(&mut self, apdu: &[u8]) -> Vec<u8> {
        let Some(header) = parse_apdu_header(apdu) else {
            return make_status(SW_WRONG_LC);
        };
        let (data, le) = extract_apdu_data(apdu);

        if header.ins != EUDS_INS_GET_RESPONSE {
            self.session.chaining_buffer = None;
        }

        match header.ins {
            EUDS_INS_SELECT => self.select(header.p1, header.p2, data),
            EUDS_INS_VERIFY => self.verify_pin(header.p1, header.p2, data),
            EUDS_INS_GET_CERT => self.get_certificate(header.p1, header.p2, le),
            EUDS_INS_GET_PUBKEY => self.get_public_key(header.p1, header.p2, le),
            EUDS_INS_GET_RESPONSE => self.get_response(le),
            EUDS_INS_SIGN => {
                if self.pin_mode == PinMode::Required && !self.session.pin_verified {
                    return make_status(SW_SECURITY_STATUS_NOT_SATISFIED);
                }
                self.sign(header.p1, header.p2, data)
            }
            EUDS_INS_DECRYPT => {
                if self.pin_mode == PinMode::Required && !self.session.pin_verified {
                    return make_status(SW_SECURITY_STATUS_NOT_SATISFIED);
                }
                self.decrypt(header.p1, header.p2, data)
            }
            _ => make_status(SW_COMMAND_NOT_ALLOWED),
        }
    }

    // ---------------------------------------------------------------------
    // SELECT Applet (INS=0xA4, CLA=0x00)
    // ---------------------------------------------------------------------
    fn select(&self, p1: u8, _p2: u8, data: &[u8]) -> Vec<u8> {
        if p1 == 0x04 && data == EUDS_AID {
            make_status(SW_SUCCESS)
        } else {
            make_status(SW_FILE_NOT_FOUND)
        }
    }

    // ---------------------------------------------------------------------
    // VERIFY PIN (INS=0xB1, CLA=0x80)
    // ---------------------------------------------------------------------
    fn verify_pin(&mut self, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
        if self.pin_mode == PinMode::NotRequired {
            return make_status(SW_SUCCESS);
        }
        if p1 != 0x00 || p2 != 0x80 {
            return make_status(SW_INVALID_P1P2);
        }
        if self.session.pin_retries == 0 {
            return make_status(SW_AUTH_METHOD_BLOCKED);
        }
        if constant_time_eq(data, self.pin.as_bytes()) {
            self.session.pin_verified = true;
            self.session.pin_retries = DEFAULT_PIN_RETRIES;
            make_status(SW_SUCCESS)
        } else {
            self.session.pin_verified = false;
            self.session.pin_retries -= 1;
            make_status(SW_VERIFY_FAILED_BASE | self.session.pin_retries as u16)
        }
    }

    // ---------------------------------------------------------------------
    // GET CERTIFICATE (INS=0xB4, CLA=0x80) — Extended APDU Case 2
    // ---------------------------------------------------------------------
    fn get_certificate(&mut self, p1: u8, p2: u8, le: Option<u16>) -> Vec<u8> {
        if p1 != 0x00 || p2 != 0x00 {
            return make_status(SW_INVALID_P1P2);
        }
        let cert = self.cert_der.clone();
        self.handle_chaining(&cert, le)
    }

    // ---------------------------------------------------------------------
    // GET PUBLIC KEY (INS=0x46, CLA=0x80) — Extended APDU Case 2
    // ---------------------------------------------------------------------
    fn get_public_key(&mut self, _p1: u8, _p2: u8, le: Option<u16>) -> Vec<u8> {
        let exp_bytes = self.e.to_bytes_be();
        let mod_bytes = self.n.to_bytes_be();

        let mut resp = Vec::with_capacity(2 + exp_bytes.len() + 2 + mod_bytes.len());
        resp.extend_from_slice(&(exp_bytes.len() as u16).to_be_bytes());
        resp.extend_from_slice(&exp_bytes);
        resp.extend_from_slice(&(mod_bytes.len() as u16).to_be_bytes());
        resp.extend_from_slice(&mod_bytes);

        self.handle_chaining(&resp, le)
    }

    // ---------------------------------------------------------------------
    // GET RESPONSE (INS=0xC0, CLA=0x80)
    // ---------------------------------------------------------------------
    fn get_response(&mut self, le: Option<u16>) -> Vec<u8> {
        if self.session.chaining_buffer.is_none() {
            return make_status(SW_WRONG_LC);
        }
        self.handle_chaining(&[], le)
    }

    // ---------------------------------------------------------------------
    // SIGN DATA (INS=0xB2, CLA=0x80)
    // ---------------------------------------------------------------------
    fn sign(&self, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
        if p1 != EUDS_SIGN_P1 || p2 != EUDS_SIGN_P2 {
            return make_status(SW_INVALID_P1P2);
        }
        match self.rsa_pkcs1_sign(data) {
            Ok(sig) => make_response(&sig, SW_SUCCESS),
            Err(e) => {
                log::error!("eUDS: RSA sign failed: {}", e);
                make_status(SW_INVALID_COMMAND_DATA)
            }
        }
    }

    // ---------------------------------------------------------------------
    // DECRYPT DATA (INS=0xB3, CLA=0x80) — Extended APDU Case 4
    // ---------------------------------------------------------------------
    fn decrypt(&self, p1: u8, p2: u8, ciphertext: &[u8]) -> Vec<u8> {
        if p1 != EUDS_DEC_P1 || p2 != EUDS_DEC_P2 {
            return make_status(SW_INVALID_P1P2);
        }
        match self.rsa_decrypt_pkcs1(ciphertext) {
            Ok(pt) => make_response(&pt, SW_SUCCESS),
            Err(e) => {
                log::error!("eUDS: RSA decrypt failed: {}", e);
                make_status(SW_INVALID_COMMAND_DATA)
            }
        }
    }

    // ---------------------------------------------------------------------
    // Response chaining for data > 256 bytes
    // ---------------------------------------------------------------------
    fn handle_chaining(&mut self, data: &[u8], le: Option<u16>) -> Vec<u8> {
        let max_chunk = if self.session.chaining_buffer.is_some() {
            match le {
                Some(v) if v > 0 => v as usize,
                _ => 256,
            }
        } else {
            256
        };

        if data.len() <= max_chunk && self.session.chaining_buffer.is_none() {
            return make_response(data, SW_SUCCESS);
        }

        if let Some(buffer) = self.session.chaining_buffer.take() {
            let take = buffer.len().min(max_chunk);
            let chunk = &buffer[..take];
            let remaining = &buffer[take..];
            if remaining.is_empty() {
                make_response(chunk, SW_SUCCESS)
            } else {
                self.session.chaining_buffer = Some(remaining.to_vec());
                let sw2 = if remaining.len() > 0xFF {
                    0x00
                } else {
                    remaining.len() as u8
                };
                make_response(chunk, SW_MORE_DATA_BASE | sw2 as u16)
            }
        } else {
            let chunk = &data[..max_chunk.min(data.len())];
            let remaining = &data[chunk.len()..];
            if remaining.is_empty() {
                make_response(chunk, SW_SUCCESS)
            } else {
                self.session.chaining_buffer = Some(remaining.to_vec());
                let sw2 = if remaining.len() > 0xFF {
                    0x00
                } else {
                    remaining.len() as u8
                };
                make_response(chunk, SW_MORE_DATA_BASE | sw2 as u16)
            }
        }
    }

    // ---------------------------------------------------------------------
    // RSA Operations
    // ---------------------------------------------------------------------
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
}
