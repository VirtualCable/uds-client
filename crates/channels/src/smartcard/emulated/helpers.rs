// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Helper functions: TLV, APDU, DER parsing, MGF1.

use num_bigint::BigUint;

// ===========================================================================
// TLV Helpers (BER-TLV encoding/decoding)
// ===========================================================================

pub fn tlv_write(buf: &mut Vec<u8>, tag: u16, data: &[u8]) {
    if tag > 0xFF {
        buf.push((tag >> 8) as u8);
        buf.push((tag & 0xFF) as u8);
    } else {
        buf.push(tag as u8);
    }
    tlv_write_length(buf, data.len());
    buf.extend_from_slice(data);
}

fn tlv_write_length(buf: &mut Vec<u8>, len: usize) {
    if len < 0x80 {
        buf.push(len as u8);
    } else if len < 0x100 {
        buf.push(0x81);
        buf.push(len as u8);
    } else {
        buf.push(0x82);
        buf.push((len >> 8) as u8);
        buf.push((len & 0xFF) as u8);
    }
}

pub fn tlv_find(data: &[u8], tag: u16) -> Option<&[u8]> {
    let mut offset = 0;
    while offset < data.len() {
        if offset >= data.len() {
            break;
        }
        let first_byte = data[offset];
        let (current_tag, tag_len) = if (first_byte & 0x1F) == 0x1F && offset + 1 < data.len() {
            (((first_byte as u16) << 8) | (data[offset + 1] as u16), 2)
        } else {
            (first_byte as u16, 1)
        };
        offset += tag_len;
        if offset >= data.len() {
            break;
        }

        let first_len_byte = data[offset];
        offset += 1;
        let value_len = if first_len_byte < 0x80 {
            first_len_byte as usize
        } else if first_len_byte == 0x81 {
            if offset >= data.len() {
                break;
            }
            let l = data[offset] as usize;
            offset += 1;
            l
        } else if first_len_byte == 0x82 {
            if offset + 1 >= data.len() {
                break;
            }
            let l = ((data[offset] as usize) << 8) | (data[offset + 1] as usize);
            offset += 2;
            l
        } else {
            break;
        };

        if current_tag == tag {
            return Some(&data[offset..offset + value_len]);
        }
        offset += value_len;
    }
    None
}

// ===========================================================================
// APDU Helpers
// ===========================================================================

pub struct ApduHeader {
    pub cla: u8,
    pub ins: u8,
    pub p1: u8,
    pub p2: u8,
}

pub fn parse_apdu_header(apdu: &[u8]) -> Option<ApduHeader> {
    if apdu.len() < 4 {
        return None;
    }
    Some(ApduHeader {
        cla: apdu[0],
        ins: apdu[1],
        p1: apdu[2],
        p2: apdu[3],
    })
}

pub fn extract_apdu_data(apdu: &[u8]) -> (&[u8], Option<u8>) {
    if apdu.len() <= 4 {
        return (&[], None);
    }
    let lc = apdu[4] as usize;
    if lc == 0 || 5 + lc > apdu.len() {
        return (&apdu[5..], None);
    }
    let data_end = 5 + lc;
    let le = if data_end < apdu.len() {
        Some(apdu[data_end])
    } else {
        None
    };
    (&apdu[5..data_end], le)
}

pub fn make_response(data: &[u8], status: u16) -> Vec<u8> {
    let mut result = Vec::with_capacity(data.len() + 2);
    result.extend_from_slice(data);
    result.push((status >> 8) as u8);
    result.push((status & 0xFF) as u8);
    result
}

pub fn make_status(status: u16) -> Vec<u8> {
    make_response(&[], status)
}

// ===========================================================================
// PKCS#1 DER Parser (minimal ASN.1)
// ===========================================================================

pub fn parse_rsa_pkcs1_components(der: &[u8]) -> Option<(BigUint, BigUint)> {
    let mut pos = 0;
    if der[pos] != 0x30 {
        return None;
    }
    pos += 1;
    pos += read_der_length(&der[pos..])?.1;
    let (_, after) = read_integer(&der[pos..])?;
    pos += after;
    let (n, after) = read_integer(&der[pos..])?;
    pos += after;
    let (_, after) = read_integer(&der[pos..])?;
    pos += after;
    let (d, _) = read_integer(&der[pos..])?;
    Some((n, d))
}

fn read_der_length(data: &[u8]) -> Option<(usize, usize)> {
    if data.is_empty() {
        return None;
    }
    let first = data[0];
    if first < 0x80 {
        Some((first as usize, 1))
    } else {
        let num_bytes = (first & 0x7F) as usize;
        if num_bytes > 4 || data.len() < 1 + num_bytes {
            return None;
        }
        let mut len = 0usize;
        for i in 0..num_bytes {
            len = (len << 8) | (data[1 + i] as usize);
        }
        Some((len, 1 + num_bytes))
    }
}

fn read_integer(data: &[u8]) -> Option<(BigUint, usize)> {
    if data.is_empty() || data[0] != 0x02 {
        return None;
    }
    let (len, len_size) = read_der_length(&data[1..])?;
    let start = 1 + len_size;
    let end = start + len;
    if end > data.len() {
        return None;
    }
    let value = if len > 0 && data[start] == 0 {
        BigUint::from_bytes_be(&data[start + 1..end])
    } else {
        BigUint::from_bytes_be(&data[start..end])
    };
    Some((value, end))
}

// ===========================================================================
// MGF1 (Mask Generation Function with SHA-1)
// ===========================================================================

pub fn mgf1(seed: &[u8], mask_len: usize) -> Vec<u8> {
    use sha1::{Digest, Sha1};
    let mut mask = Vec::with_capacity(mask_len);
    let mut counter: u32 = 0;
    while mask.len() < mask_len {
        let mut hasher = Sha1::new();
        hasher.update(seed);
        hasher.update(counter.to_be_bytes());
        mask.extend_from_slice(&hasher.finalize());
        counter += 1;
    }
    mask.truncate(mask_len);
    mask
}
