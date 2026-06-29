// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Helper functions: TLV, APDU, DER parsing, MGF1.

use num_bigint::BigUint;

// ===========================================================================
// APDU Helpers
// ===========================================================================

pub struct ApduHeader {
    pub ins: u8,
    pub p1: u8,
    pub p2: u8,
}

pub fn parse_apdu_header(apdu: &[u8]) -> Option<ApduHeader> {
    if apdu.len() < 4 {
        return None;
    }
    Some(ApduHeader {
        ins: apdu[1],
        p1: apdu[2],
        p2: apdu[3],
    })
}

pub fn extract_apdu_data(apdu: &[u8]) -> (&[u8], Option<u16>) {
    let len = apdu.len();
    if len <= 4 {
        return (&[], None);
    }
    let b4 = apdu[4];
    if b4 != 0 {
        let lc = b4 as usize;
        if 5 + lc <= len {
            let data = &apdu[5..5 + lc];
            let le = if 5 + lc + 1 <= len {
                let v = apdu[5 + lc] as u16;
                Some(if v == 0 { 256 } else { v })
            } else {
                None
            };
            (data, le)
        } else {
            let le = b4 as u16;
            (&[], Some(if le == 0 { 256 } else { le }))
        }
    } else {
        if len < 7 {
            return (&[], None);
        }
        let ext = ((apdu[5] as usize) << 8) | (apdu[6] as usize);
        if 7 + ext <= len {
            let data = &apdu[7..7 + ext];
            let data_end = 7 + ext;
            let le = if data_end + 2 <= len {
                let v = ((apdu[data_end] as u16) << 8) | (apdu[data_end + 1] as u16);
                Some(v)
            } else if data_end < len {
                let v = apdu[data_end] as u16;
                Some(if v == 0 { 256 } else { v })
            } else {
                None
            };
            (data, le)
        } else {
            let le = ext as u16;
            (&[], Some(le))
        }
    }
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

pub fn parse_rsa_pkcs1_components(der: &[u8]) -> Option<(BigUint, BigUint, BigUint)> {
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
    let (e, after) = read_integer(&der[pos..])?;
    pos += after;
    let (d, _) = read_integer(&der[pos..])?;
    Some((n, e, d))
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

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}
