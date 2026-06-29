// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! eUDS constants — APDU instructions, status words, ATR, AID.

// ============================================================================
// eUDS Custom APDU Protocol Constants
// ============================================================================

// APDU Instructions
pub const EUDS_INS_SELECT: u8 = 0xA4;     // Standard SELECT (CLA=0x00)
pub const EUDS_INS_VERIFY: u8 = 0xB1;     // Proprietary: VERIFY PIN
pub const EUDS_INS_GET_CERT: u8 = 0xB4;   // Proprietary: GET CERTIFICATE
pub const EUDS_INS_GET_PUBKEY: u8 = 0x46;  // Proprietary: GET PUBLIC KEY
pub const EUDS_INS_GET_RESPONSE: u8 = 0xC0; // GET RESPONSE (chaining)
pub const EUDS_INS_SIGN: u8 = 0xB2;       // Proprietary: SIGN DATA
pub const EUDS_INS_DECRYPT: u8 = 0xB3;     // Proprietary: DECRYPT DATA

// SIGN/DECRYPT P1/P2
pub const EUDS_SIGN_P1: u8 = 0x9E;
pub const EUDS_SIGN_P2: u8 = 0x9A;
pub const EUDS_DEC_P1: u8 = 0x80;
pub const EUDS_DEC_P2: u8 = 0x86;

// eUDS Custom AID (9 bytes)
pub const EUDS_AID: &[u8] = b"eUDS-Card"; // 65 75 44 53 2D 43 61 72 64

// Status Words
pub const SW_SUCCESS: u16 = 0x9000;
pub const SW_MORE_DATA_BASE: u16 = 0x6100; // 61 XX
pub const SW_WRONG_LC: u16 = 0x6700;
pub const SW_COMMAND_NOT_ALLOWED: u16 = 0x6986;
pub const SW_SECURITY_STATUS_NOT_SATISFIED: u16 = 0x6982;
pub const SW_AUTH_METHOD_BLOCKED: u16 = 0x6983;
pub const SW_VERIFY_FAILED_BASE: u16 = 0x63C0; // 63 CX
pub const SW_FILE_NOT_FOUND: u16 = 0x6A82;
pub const SW_INVALID_P1P2: u16 = 0x6A86;
pub const SW_INVALID_COMMAND_DATA: u16 = 0x6A80;

// eUDS ATR (ISO 7816-3, T=1 protocol)
// TS=3B, T0=89 (Y1=8, K=9), TD1=01 (T=1), H="eUDS-Card", TCK=96
pub const EUDS_ATR: &[u8] = &[
    0x3B, 0x89, 0x01, 0x45, 0x55, 0x44, 0x53, 0x2D, 0x43, 0x61, 0x72, 0x64, 0x96
];

// Reader name
pub const EUDS_READER_NAME: &str = "eUDS Virtual Smartcard Reader";

// PIN
pub const DEFAULT_PIN_RETRIES: u8 = 3;
