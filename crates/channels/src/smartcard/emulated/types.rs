// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Internal types for the GIDS engine.

use num_bigint::BigUint;
use std::collections::HashMap;

use super::consts::*;
use super::helpers::tlv_find;

use std::io::Write;

/// RSA padding modes for decrypt operations.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum RsaPadding {
    Pkcs1v15,
    Oaep,
    Raw,
}

/// Security environment configured by MSE.
#[derive(Clone, Copy)]
pub struct SecurityEnvironment {
    pub crt: u8,
    pub algo_id: u8,
    pub key_ref: u8,
}

/// Session state for a smartcard context.
pub struct SessionState {
    pub current_df: u16,
    pub pin_verified: bool,
    pub pin_retries: u8,
    pub current_se: Option<SecurityEnvironment>,
    pub response_buffer: Vec<u8>,
    pub command_buffer: Vec<u8>,
}

impl SessionState {
    pub fn new() -> Self {
        SessionState {
            current_df: 0,
            pin_verified: false,
            pin_retries: DEFAULT_PIN_RETRIES,
            current_se: None,
            response_buffer: Vec::new(),
            command_buffer: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.current_df = 0;
        self.pin_verified = false;
        self.pin_retries = DEFAULT_PIN_RETRIES;
        self.current_se = None;
        self.response_buffer.clear();
        self.command_buffer.clear();
    }
}

/// Virtual filesystem for the GIDS card.
pub struct VirtualFs {
    /// EF ID → TLV-encoded data (sequence of DOs)
    files: HashMap<u16, Vec<u8>>,
}

impl VirtualFs {
    pub fn new(cert_der: &[u8], _n: &BigUint, _d: &BigUint, key_bytes: usize) -> Self {
        use super::helpers::tlv_write;

        let mut files = HashMap::new();

        let mut master_data = Vec::new();
        tlv_write(
            &mut master_data,
            DO_FILESYSTEM_TABLE,
            &Self::build_fs_table(),
        );
        tlv_write(&mut master_data, DO_KEYMAP, &Self::build_keymap(key_bytes));
        files.insert(EFID_MASTER, master_data);

        let mut cardid_data = Vec::new();
        let cardid: Vec<u8> = (0..16).map(|_| rand::random::<u8>()).collect();
        tlv_write(&mut cardid_data, DO_CARDID, &cardid);
        files.insert(EFID_CARDID, cardid_data);

        let mut common_data = Vec::new();
        tlv_write(&mut common_data, DO_CARDAPPS, &CARDAPPS_CONTENT);
        tlv_write(&mut common_data, DO_CARDCF, &CARDCF_CONTENT);
        tlv_write(
            &mut common_data,
            DO_CMAPFILE,
            &Self::build_cmap_file(key_bytes),
        );
        tlv_write(&mut common_data, DO_KXC00, &Self::build_kxc00(cert_der));
        files.insert(EFID_COMMON, common_data);

        VirtualFs { files }
    }

    pub fn read_do(&self, ef_id: u16, do_id: u16) -> Option<&[u8]> {
        self.files
            .get(&ef_id)
            .and_then(|data| tlv_find(data, do_id))
    }

    /// Get the raw TLV data of an entire EF.
    pub fn get_ef_data(&self, ef_id: u16) -> Option<&[u8]> {
        self.files.get(&ef_id).map(|v| v.as_slice())
    }

    fn build_fs_table() -> Vec<u8> {
        fn entry(dir: &str, name: &str, do_id: u16, ef_id: u16) -> [u8; 28] {
            let mut buf = [0u8; 28];
            let dir_bytes = dir.as_bytes();
            let name_bytes = name.as_bytes();
            buf[..dir_bytes.len().min(9)].copy_from_slice(&dir_bytes[..dir_bytes.len().min(9)]);
            buf[9..9 + name_bytes.len().min(9)]
                .copy_from_slice(&name_bytes[..name_bytes.len().min(9)]);
            buf[20] = (do_id & 0xFF) as u8;
            buf[21] = (do_id >> 8) as u8;
            buf[24] = (ef_id & 0xFF) as u8;
            buf[25] = (ef_id >> 8) as u8;
            buf
        }

        let entries: [[u8; 28]; 6] = [
            entry("mscp", "", 0x0000, EFID_MASTER),
            entry("", "cardid", DO_CARDID, EFID_CARDID),
            entry("", "cardapps", DO_CARDAPPS, EFID_COMMON),
            entry("", "cardcf", DO_CARDCF, EFID_COMMON),
            entry("mscp", "cmapfile", DO_CMAPFILE, EFID_COMMON),
            entry("mscp", "kxc00", DO_KXC00, EFID_COMMON),
        ];
        let mut buf = Vec::with_capacity(1 + entries.len() * 28);
        buf.push(entries.len() as u8);
        for e in &entries {
            buf.extend_from_slice(e);
        }
        buf
    }

    fn build_keymap(key_bits: usize) -> Vec<u8> {
        let algid = match key_bits {
            128 => ALGID_RSA_1024,
            256 => ALGID_RSA_2048,
            384 => ALGID_RSA_3072,
            512 => ALGID_RSA_4096,
            _ => ALGID_RSA_2048,
        };
        let mut record = [0u8; 12];
        record[0] = 0x01;
        record[4] = algid;
        record[5] = KEY_TYPE_KEYEXCHANGE;
        record[6] = DEFAULT_KEY_REF;
        record[7] = 0xB0;
        record[8] = 0xFF;
        record[9] = 0xFF;
        let mut buf = Vec::with_capacity(1 + 12);
        buf.push(1);
        buf.extend_from_slice(&record);
        buf
    }

    fn build_cmap_file(key_size_bytes: usize) -> Vec<u8> {
        let name: Vec<u16> = "Private Key 00"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let name_bytes: Vec<u8> = name.iter().flat_map(|c| c.to_le_bytes()).collect();
        let key_bits = (key_size_bytes * 8) as u32;

        let mut buf = Vec::new();
        buf.extend_from_slice(&3u32.to_le_bytes());
        buf.extend_from_slice(&key_bits.to_le_bytes());
        buf.extend_from_slice(&key_bits.to_le_bytes());
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes());
        buf.extend_from_slice(&((name.len() - 1) as u32).to_le_bytes());
        buf.extend_from_slice(&name_bytes);
        buf
    }

    fn build_kxc00(cert_der: &[u8]) -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::ZlibEncoder;
        let mut compressed = Vec::new();
        {
            let mut encoder = ZlibEncoder::new(&mut compressed, Compression::default());
            encoder.write_all(cert_der).unwrap();
            encoder.finish().unwrap();
        }
        let src_len = cert_der.len().min(0xFFFF) as u16;
        let mut buf = Vec::with_capacity(4 + compressed.len());
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&src_len.to_le_bytes());
        buf.extend_from_slice(&compressed);
        buf
    }
}
