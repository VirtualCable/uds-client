// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! GIDS card emulation engine.
//!
//! Emulates the GIDS (Generic Identity Device Specification) protocol that msclmd
//! — the Windows GIDS minidriver — drives when the reference card is used. The APDU
//! flow and responses replicate the reference card captured during testing (see
//! `docs/smartcard-connect-phase-discovery.md`): SELECT, GET DATA (2F01, 7F73, DF1F,
//! DF20, DF22, DF23, 7F49, DF24), VERIFY, MSE SET and PSO.
//!
//! The key type is detected on load: **RSA** (PKCS#1 v1.5 sign, `7F49` = modulus +
//! exponent) or **ECDSA** P-256/P-384/P-521 (`7F49` = `86` uncompressed point, PSO
//! returns the GIDS-spec `SEQUENCE { r, s }` DER signature over the precomputed
//! hash). The public key (`7F 49`), certificate (`DF 24`), card id and container GUID
//! are derived from the loaded certificate + private key.
//!
//! PIN handling mirrors a real smartcard: the certificate (public) is served without
//! any PIN. Signing is gated by a VERIFY. The "PIN" is the private key's password
//! when the key is encrypted: VERIFY succeeds only if the entered PIN decrypts the
//! key. If the key has no password, no PIN is required at all.

use std::io::Write;
use std::sync::LazyLock;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use num_bigint::BigUint;
use p256::ecdsa::SigningKey as P256SigningKey;
use p384::ecdsa::SigningKey as P384SigningKey;
use p521::ecdsa::SigningKey as P521SigningKey;
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::pkcs8::DecodePrivateKey;
use rsa::RsaPrivateKey;

use super::helpers::*;

// ============================================================================
// GIDS protocol constants (from the reference card)
// ============================================================================

/// GIDS applet AID (9 bytes).
pub const GIDS_AID: &[u8] = &[0xA0, 0x00, 0x00, 0x03, 0x97, 0x42, 0x54, 0x46, 0x59];

/// ATR that Windows maps to the GIDS minidriver (reuses the reference card ATR so
/// the remote selects msclmd exactly like with the physical card).
pub const GIDS_ATR: &[u8] = &[
    0x3B, 0xF9, 0x13, 0x00, 0x00, 0x81, 0x31, 0xFE, 0x45, 0x4A, 0x43, 0x4F, 0x50, 0x32, 0x34,
    0x32, 0x52, 0x33, 0xA2,
];

/// Reader name presented to the remote.
pub const GIDS_READER_NAME: &str = "GIDS Virtual Smartcard Reader";

/// Card identifier (GET DATA DF20, 16 bytes) — the card GUID that identifies the
/// card type and drives the OS-cache key. Reuses the reference card's identifier
/// so the emulated card is recognized exactly like the physical GIDS card.
pub const REFERENCE_CARDID: [u8; 16] = [
    0x2F, 0xB6, 0x08, 0x59, 0x09, 0xA5, 0xBA, 0x95, 0xEE, 0x5D, 0x78, 0xF7, 0x86, 0xB6, 0x0B, 0xC8,
];

/// Container GUID (cmapfile) — from the reference card. Maps the card to the
/// container used by certutil; kept stable so Windows associates the same container.
pub const REFERENCE_GUID: &str = "tq-00eb5be7-d2bd-48f8-9211-b0faddbf2788";

const SW_SUCCESS: u16 = 0x9000;
const SW_MORE_DATA: u16 = 0x6100;
const SW_WRONG_LC: u16 = 0x6700;
const SW_COMMAND_NOT_ALLOWED: u16 = 0x6986;
const SW_SECURITY_STATUS_NOT_SATISFIED: u16 = 0x6982;
const SW_AUTH_METHOD_BLOCKED: u16 = 0x6983;
const SW_VERIFY_FAILED: u16 = 0x63C0;
const SW_FILE_NOT_FOUND: u16 = 0x6A88;
const SW_INVALID_P1P2: u16 = 0x6A86;
const SW_F_INTERNAL_ERROR: u16 = 0x6581;

/// SELECT GIDS AID (P2=00) response: FCI with the AID (Application Template).
const SELECT_FCI: &[u8] = &[
    0x61, 0x12, 0x4F, 0x0B, 0xA0, 0x00, 0x00, 0x03, 0x97, 0x42, 0x54, 0x46, 0x59, 0x02, 0x01,
    0x73, 0x03, 0x40, 0x01, 0xC0,
];

/// SELECT GIDS AID with P2=04 response (warning-style FCI from the reference card).
const SELECT_FCI_P204: &[u8] = &[0x62, 0x08, 0x82, 0x01, 0x38, 0x8C, 0x03, 0x03, 0x30, 0x30];

/// Applet identifying info (GET DATA 2F01) — "MySmartLogon".
const APPLET_INFO: &[u8] = &[
    0x43, 0x01, 0xF4, 0x47, 0x03, 0x08, 0x01, 0x80, 0x46, 0x0C, 0x4D, 0x79, 0x53, 0x6D, 0x61,
    0x72, 0x74, 0x4C, 0x6F, 0x67, 0x6F, 0x6E,
];

/// GET DATA DF20 at P1=A000 (card property / PIN info) from the reference card.
const CARD_PIN_INFO: &[u8] = &[
    0xDF, 0x20, 0x0D, 0x01, 0x01, 0x00, 0x00, 0x00, 0x07, 0x9A, 0x81, 0xB0, 0xFF, 0xFF, 0x00,
    0x00,
];

/// Card configuration (GET DATA DF22) from the reference card.
const CARD_CONFIG: &[u8] = &[0xDF, 0x22, 0x06, 0x00, 0x00, 0x01, 0x00, 0x05, 0x00];

/// Card application table (GET DATA DF1F) from the reference card. Maps the logical
/// files (cardid, cardapps, cardcf, cmapfile, kxc00) to their P1 + tag references.
const CARDAPPS_HEX: &str = concat!(
    "DF1F81A901",
    "6D736370000000000000000000000000000000000000000000A00000000000000000000000",
    "636172646964000000000020DF000012A00000000000000000000000",
    "636172646170707300000021DF000010A00000000000000000000000",
    "636172646366000000000022DF000010A000006D7363700000000000",
    "636D617066696C6500000023DF000010A000006D7363700000000000",
    "6B7863303000000000000024DF000010A00000",
);

fn bytes_from_hex(hex: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(hex.len() / 2);
    let mut iter = hex.bytes();
    while let (Some(hi), Some(lo)) = (iter.next(), iter.next()) {
        let hi = (hi as char).to_digit(16).unwrap_or(0) as u8;
        let lo = (lo as char).to_digit(16).unwrap_or(0) as u8;
        out.push((hi << 4) | lo);
    }
    out
}

static CARDAPPS: LazyLock<Vec<u8>> = LazyLock::new(|| bytes_from_hex(CARDAPPS_HEX));

// ============================================================================
// GIDS engine
// ============================================================================

/// How the "PIN" gates the private key:
/// - `None`: the private key has no password -> signing needs no PIN.
/// - `KeyPassword`: the private key is encrypted; the PIN IS its password. VERIFY
///   succeeds only if the entered PIN actually decrypts the key.
#[derive(Clone, Copy, PartialEq, Debug)]
enum PinMode {
    None,
    KeyPassword,
}

/// ECC curve supported by the emulator (GIDS mech refs `0C`/`0D`/`0E`).
#[derive(Clone, Copy, PartialEq, Debug)]
enum EcCurve {
    P256,
    P384,
    P521,
}

/// ECDSA signing key (private), curve-erased over the supported curves.
enum EcSigningKey {
    P256(P256SigningKey),
    P384(P384SigningKey),
    P521(P521SigningKey),
}

impl EcSigningKey {
    fn curve(&self) -> EcCurve {
        match self {
            EcSigningKey::P256(_) => EcCurve::P256,
            EcSigningKey::P384(_) => EcCurve::P384,
            EcSigningKey::P521(_) => EcCurve::P521,
        }
    }

    /// Uncompressed public point `04 || X || Y` derived from the private key.
    fn public_point(&self) -> Vec<u8> {
        match self {
            EcSigningKey::P256(k) => {
                p256::ecdsa::VerifyingKey::from(k).to_encoded_point(false).as_bytes().to_vec()
            }
            EcSigningKey::P384(k) => {
                p384::ecdsa::VerifyingKey::from(k).to_encoded_point(false).as_bytes().to_vec()
            }
            EcSigningKey::P521(k) => {
                p521::ecdsa::VerifyingKey::from(k).to_encoded_point(false).as_bytes().to_vec()
            }
        }
    }

    /// ECDSA signature over a precomputed hash, returned as the GIDS-spec
    /// `EC Signature ::= SEQUENCE { r INTEGER, s INTEGER }` (DER).
    fn sign(&self, hash: &[u8]) -> Result<Vec<u8>, String> {
        use p256::ecdsa::signature::hazmat::PrehashSigner;
        match self {
            EcSigningKey::P256(k) => {
                let sig: p256::ecdsa::Signature = k.sign_prehash(hash).map_err(|e| e.to_string())?;
                Ok(sig.to_der().as_bytes().to_vec())
            }
            EcSigningKey::P384(k) => {
                let sig: p384::ecdsa::Signature = k.sign_prehash(hash).map_err(|e| e.to_string())?;
                Ok(sig.to_der().as_bytes().to_vec())
            }
            EcSigningKey::P521(k) => {
                let sig: p521::ecdsa::Signature = k.sign_prehash(hash).map_err(|e| e.to_string())?;
                Ok(sig.to_der().as_bytes().to_vec())
            }
        }
    }
}

/// Key material held by the engine, erased over RSA / ECDSA.
enum KeyType {
    Rsa {
        n: BigUint,
        e: BigUint,
        /// Private exponent; `None` until an encrypted key is decrypted by VERIFY.
        d: Option<BigUint>,
        key_size: usize,
    },
    Ec {
        curve: EcCurve,
        /// Uncompressed public point `04 || X || Y` (from the key, or the SPKI when
        /// the key is encrypted).
        point: Vec<u8>,
        /// Private signing key; `None` until an encrypted key is decrypted by VERIFY.
        key: Option<EcSigningKey>,
    },
}

impl KeyType {
    fn name(&self) -> &'static str {
        match self {
            KeyType::Rsa { .. } => "RSA",
            KeyType::Ec { curve, .. } => match curve {
                EcCurve::P256 => "ECDSA P-256",
                EcCurve::P384 => "ECDSA P-384",
                EcCurve::P521 => "ECDSA P-521",
            },
        }
    }
}

/// Public key extracted from an X.509 certificate SPKI (used when the private key
/// is encrypted: the public part must be served without PIN during discovery).
enum SpkiPublic {
    Rsa { n: BigUint, e: BigUint },
    Ec { curve: EcCurve, point: Vec<u8> },
}

pub struct GidsEngine {
    /// `Cached_GeneralFile/mscp/kxcXX` content: `01 00` + uncompressed cert length
    /// (2 bytes LITTLE-ENDIAN) + zlib-compressed certificate DER. The BaseCSP
    /// expects this compressed format (the reference card stores it this way).
    cert_content: Vec<u8>,
    pin_mode: PinMode,
    /// Encrypted PKCS#8 PEM of the private key, kept to decrypt on VERIFY. `None`
    /// when the key is not encrypted.
    key_pem: Option<String>,
    /// Key material (RSA or ECDSA). The private part is `None` until a correct
    /// VERIFY decrypts an encrypted key (always present when `pin_mode == None`).
    key: KeyType,
    pin_verified: bool,
    pin_retries: u8,
    chaining: Option<Vec<u8>>,
}

impl std::fmt::Debug for GidsEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GidsEngine")
            .field("key_type", &self.key.name())
            .field("pin_mode", &self.pin_mode)
            .field("pin_verified", &self.pin_verified)
            .finish()
    }
}

const DEFAULT_PIN_RETRIES: u8 = 3;

impl GidsEngine {
    pub fn new(cert_der: Vec<u8>, key_pem: String) -> Result<Self, String> {
        // Compress the certificate the same way the reference card stores it:
        // `01 00` + uncompressed length (2 bytes LITTLE-ENDIAN) + zlib(flate)
        // compressed DER. The reference card: `01 00 FC 03` = len 0x03FC (1020).
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&cert_der).map_err(|e| format!("compress cert: {e}"))?;
        let compressed = encoder.finish().map_err(|e| format!("zlib finish: {e}"))?;
        let mut cert_content = Vec::with_capacity(4 + compressed.len());
        cert_content.extend_from_slice(&[0x01, 0x00]);
        cert_content.extend_from_slice(&(cert_der.len() as u16).to_le_bytes());
        cert_content.extend_from_slice(&compressed);

        // NOTE: the cardid and container GUID are FIXED to the reference card's
        // values. The cardid identifies the card type (and thus how Windows/msclmd
        // treats it), so an arbitrary value could make the card look like a
        // different/unknown type. The ATR + AID + cardid together reproduce the
        // reference GIDS card; only the crypto material (7F49, DF24, sign) differs.

        // Try the private key WITHOUT a password (unencrypted): RSA first, then the
        // ECDSA curves. If all fail the key is encrypted and its password will act
        // as the card PIN (public part then comes from the certificate).
        if let Ok(key) = RsaPrivateKey::from_pkcs8_pem(&key_pem) {
            let pkcs1 = key
                .to_pkcs1_der()
                .map_err(|e| format!("serialize key: {e}"))?;
            let (n, e, d) = parse_rsa_pkcs1_components(pkcs1.as_bytes())
                .ok_or_else(|| "parse key components".to_string())?;
            let key_size = (n.bits() as usize).div_ceil(8);
            return Ok(GidsEngine {
                cert_content,
                pin_mode: PinMode::None,
                key_pem: None,
                key: KeyType::Rsa { n, e, d: Some(d), key_size },
                pin_verified: true,
                pin_retries: DEFAULT_PIN_RETRIES,
                chaining: None,
            });
        }

        if let Some(signing_key) = parse_ec_signing_key_pem(&key_pem) {
            let curve = signing_key.curve();
            let point = signing_key.public_point();
            return Ok(GidsEngine {
                cert_content,
                pin_mode: PinMode::None,
                key_pem: None,
                key: KeyType::Ec { curve, point, key: Some(signing_key) },
                pin_verified: true,
                pin_retries: DEFAULT_PIN_RETRIES,
                chaining: None,
            });
        }

        // Encrypted key: the public part must come from the certificate (it is
        // served without PIN in the 7F49 during discovery).
        let key = match extract_spki_public(&cert_der)? {
            SpkiPublic::Rsa { n, e } => {
                let key_size = (n.bits() as usize).div_ceil(8);
                KeyType::Rsa { n, e, d: None, key_size }
            }
            SpkiPublic::Ec { curve, point } => KeyType::Ec { curve, point, key: None },
        };
        log::info!(
            "GIDS: private key is encrypted; PIN = key password (verified by decrypting the key)"
        );
        Ok(GidsEngine {
            cert_content,
            pin_mode: PinMode::KeyPassword,
            key_pem: Some(key_pem),
            key,
            pin_verified: false,
            pin_retries: DEFAULT_PIN_RETRIES,
            chaining: None,
        })
    }

    /// Process a raw APDU and return the raw response (data + SW1SW2).
    pub fn process_apdu(&mut self, apdu: &[u8]) -> Vec<u8> {
        let Some(header) = parse_apdu_header(apdu) else {
            return make_status(SW_WRONG_LC);
        };
        let (data, le) = extract_apdu_data(apdu);

        if header.ins != 0xC0 {
            self.chaining = None;
        }

        match header.ins {
            0xA4 => self.select(header.p1, header.p2, data),
            0xCB => self.get_data(header.p1, header.p2, data, le),
            0xC0 => self.get_response(le),
            0x20 => self.verify(header.p1, header.p2, data),
            0x22 => self.mse_set(),
            0x2A => self.pso_sign(header.p1, header.p2, data),
            _ => make_status(SW_COMMAND_NOT_ALLOWED),
        }
    }

    // ---------------------------------------------------------------------
    // SELECT (INS=0xA4)
    // ---------------------------------------------------------------------
    fn select(&self, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
        match p1 {
            0x04 => {
                if data == GIDS_AID {
                    if p2 == 0x04 {
                        make_response(SELECT_FCI_P204, SW_SUCCESS)
                    } else {
                        make_response(SELECT_FCI, SW_SUCCESS)
                    }
                } else {
                    // Reference card returns 6A 82 (file not found) for unknown AIDs.
                    make_status(0x6A82)
                }
            }
            0x00 => {
                // SELECT by DF name (e.g. 3F 00 for the MF).
                make_status(SW_SUCCESS)
            }
            _ => make_status(SW_INVALID_P1P2),
        }
    }

    // ---------------------------------------------------------------------
    // GET DATA (INS=0xCB)
    // ---------------------------------------------------------------------
    fn get_data(&mut self, p1: u8, p2: u8, data: &[u8], le: Option<u16>) -> Vec<u8> {
        if p1 == 0x2F {
            return make_response(APPLET_INFO, SW_SUCCESS);
        }

        let Some((thi, tlo)) = find_gids_tag(data) else {
            return make_status(SW_FILE_NOT_FOUND);
        };

        match (thi, tlo) {
            (0xDF, 0x1F) => make_response(&CARDAPPS, SW_SUCCESS),
            (0xDF, 0x20) => {
                if p1 == 0xA0 && p2 == 0x00 {
                    make_response(CARD_PIN_INFO, SW_SUCCESS)
                } else {
                    // A0 12 → card id
                    let mut resp = Vec::with_capacity(19);
                    resp.extend_from_slice(&[0xDF, 0x20, 0x10]);
                    resp.extend_from_slice(&REFERENCE_CARDID);
                    make_response(&resp, SW_SUCCESS)
                }
            }
            (0xDF, 0x22) => make_response(CARD_CONFIG, SW_SUCCESS),
            (0xDF, 0x23) => {
                // CONTAINERMAPRECORD: GUID + flags (VALID|DEFAULT) + wSigKeySizeBits +
                // wKeyExchangeKeySizeBits (little-endian). The key size must match the
                // actual key: msclmd rejects the container when the 7F49 key does not
                // match the size declared here (2048 for the RSA reference card).
                let key_bits = self.container_key_size_bits();
                let mut resp = Vec::with_capacity(89);
                resp.extend_from_slice(&[0xDF, 0x23, 0x56]);
                for unit in REFERENCE_GUID.encode_utf16() {
                    resp.extend_from_slice(&unit.to_le_bytes());
                }
                resp.extend_from_slice(&[0x00, 0x00, 0x03, 0x00, 0x00, 0x00]);
                resp.extend_from_slice(&key_bits.to_le_bytes());
                make_response(&resp, SW_SUCCESS)
            }
            (0xDF, 0x24) => {
                // Wrap the (compressed) certificate content in the `DF 24` DO.
                let content = &self.cert_content;
                let mut do_ = Vec::with_capacity(5 + content.len());
                do_.extend_from_slice(&[0xDF, 0x24]);
                match content.len() {
                    len if len < 0x80 => do_.push(len as u8),
                    len if len < 0x100 => {
                        do_.extend_from_slice(&[0x81, len as u8]);
                    }
                    len => {
                        do_.extend_from_slice(&[0x82, (len >> 8) as u8, (len & 0xFF) as u8]);
                    }
                }
                do_.extend_from_slice(content);
                self.handle_chaining(&do_, le)
            }
            (0x7F, 0x73) => make_status(SW_FILE_NOT_FOUND),
            (0x7F, 0x49) => {
                let pk = self.pubkey_do();
                self.handle_chaining(&pk, le)
            }
            _ => make_status(SW_FILE_NOT_FOUND),
        }
    }

    // ---------------------------------------------------------------------
    // GET RESPONSE (INS=0xC0)
    // ---------------------------------------------------------------------
    fn get_response(&mut self, le: Option<u16>) -> Vec<u8> {
        if self.chaining.is_none() {
            return make_status(SW_WRONG_LC);
        }
        self.handle_chaining(&[], le)
    }

    // ---------------------------------------------------------------------
    // VERIFY (INS=0x20)
    //
    // The PIN is the private key's password when the key is encrypted: VERIFY
    // succeeds only if the entered PIN actually decrypts the key. If the key has
    // no password, any VERIFY succeeds (the card needs no PIN).
    // ---------------------------------------------------------------------
    fn verify(&mut self, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
        if p1 != 0x00 {
            return make_status(SW_INVALID_P1P2);
        }
        match p2 {
            0x80 => {
                if self.pin_retries == 0 {
                    return make_status(SW_AUTH_METHOD_BLOCKED);
                }
                match self.pin_mode {
                    PinMode::None => {
                        self.pin_verified = true;
                        make_status(SW_SUCCESS)
                    }
                    PinMode::KeyPassword => {
                        let pem = self.key_pem.clone().unwrap();
                        let pin = String::from_utf8_lossy(data);
                        let ok = match &mut self.key {
                            KeyType::Rsa { d, .. } => match RsaPrivateKey::from_pkcs8_encrypted_pem(
                                &pem,
                                pin.as_bytes(),
                            ) {
                                Ok(key) => {
                                    let pkcs1 = match key.to_pkcs1_der() {
                                        Ok(p) => p,
                                        Err(_) => return make_status(SW_F_INTERNAL_ERROR),
                                    };
                                    match parse_rsa_pkcs1_components(pkcs1.as_bytes()) {
                                        Some((_, _, priv_d)) => {
                                            *d = Some(priv_d);
                                            true
                                        }
                                        None => false,
                                    }
                                }
                                Err(_) => false,
                            },
                            KeyType::Ec { curve, key, .. } => {
                                match decrypt_ec_signing_key_pem(*curve, &pem, pin.as_bytes()) {
                                    Some(k) => {
                                        *key = Some(k);
                                        true
                                    }
                                    None => false,
                                }
                            }
                        };
                        if ok {
                            self.pin_verified = true;
                            self.pin_retries = DEFAULT_PIN_RETRIES;
                            make_status(SW_SUCCESS)
                        } else {
                            self.pin_verified = false;
                            self.pin_retries -= 1;
                            make_status(SW_VERIFY_FAILED | self.pin_retries as u16)
                        }
                    }
                }
            }
            // Second PIN slot (unused on the reference card; msclmd probes it).
            0x82 => make_status(SW_SUCCESS),
            _ => make_status(SW_INVALID_P1P2),
        }
    }

    // ---------------------------------------------------------------------
    // MSE SET (INS=0x22) — select a key reference for subsequent PSO.
    // ---------------------------------------------------------------------
    fn mse_set(&self) -> Vec<u8> {
        make_status(SW_SUCCESS)
    }

    // ---------------------------------------------------------------------
    // PSO: COMPUTE DIGITAL SIGNATURE (INS=0x2A, P1=0x9E)
    //
    // RSA: PKCS#1 v1.5 over the supplied DigestInfo; response is the raw
    //      256/384/512-byte signature.
    // ECDSA: signs the supplied precomputed hash; response is the GIDS-spec
    //       `EC Signature ::= SEQUENCE { r INTEGER, s INTEGER }` (DER).
    // ---------------------------------------------------------------------
    fn pso_sign(&self, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
        if p1 != 0x9E {
            return make_status(SW_INVALID_P1P2);
        }
        if p2 != 0x9A && p2 != 0x80 {
            return make_status(SW_INVALID_P1P2);
        }
        if !self.pin_verified {
            return make_status(SW_SECURITY_STATUS_NOT_SATISFIED);
        }
        match &self.key {
            KeyType::Rsa { n, d, key_size, .. } => {
                let d = match d {
                    Some(d) => d,
                    None => return make_status(SW_SECURITY_STATUS_NOT_SATISFIED),
                };
                match Self::rsa_pkcs1_sign(n, d, *key_size, data) {
                    Ok(sig) => make_response(&sig, SW_SUCCESS),
                    Err(_) => make_status(SW_COMMAND_NOT_ALLOWED),
                }
            }
            KeyType::Ec { key, .. } => {
                let key = match key {
                    Some(k) => k,
                    None => return make_status(SW_SECURITY_STATUS_NOT_SATISFIED),
                };
                match key.sign(data) {
                    Ok(sig) => make_response(&sig, SW_SUCCESS),
                    Err(_) => make_status(SW_COMMAND_NOT_ALLOWED),
                }
            }
        }
    }

    // ---------------------------------------------------------------------
    // Response chaining for data > Le (61 XX / GET RESPONSE)
    // ---------------------------------------------------------------------
    fn handle_chaining(&mut self, data: &[u8], le: Option<u16>) -> Vec<u8> {
        let chunk_size = match le {
            Some(v) if v > 0 => v as usize,
            _ => 256,
        };

        let available: &[u8] = match self.chaining.take() {
            Some(buf) => {
                let rest = &buf[buf.len().min(chunk_size)..];
                if rest.is_empty() {
                    return make_response(&buf[..buf.len().min(chunk_size)], SW_SUCCESS);
                }
                self.chaining = Some(rest.to_vec());
                let sw2 = if rest.len() > 0xFF { 0x00 } else { rest.len() as u8 };
                return make_response(&buf[..buf.len().min(chunk_size)], SW_MORE_DATA | sw2 as u16);
            }
            None => data,
        };

        if available.len() <= chunk_size {
            return make_response(available, SW_SUCCESS);
        }

        let chunk = &available[..chunk_size];
        let remaining = &available[chunk_size..];
        self.chaining = Some(remaining.to_vec());
        let sw2 = if remaining.len() > 0xFF { 0x00 } else { remaining.len() as u8 };
        make_response(chunk, SW_MORE_DATA | sw2 as u16)
    }

    // ---------------------------------------------------------------------
    // Data builders
    // ---------------------------------------------------------------------

    /// Container key size in bits, declared in the `cmapfile` (DF23). Must match
    /// the actual key type: 2048 for the RSA reference card, the curve size for
    /// ECDSA (256/384/521).
    fn container_key_size_bits(&self) -> u16 {
        match &self.key {
            KeyType::Rsa { key_size, .. } => (key_size * 8) as u16,
            KeyType::Ec { curve, .. } => match curve {
                EcCurve::P256 => 256,
                EcCurve::P384 => 384,
                EcCurve::P521 => 521,
            },
        }
    }

    /// The `7F 49` public-key DO.
    /// RSA: `81` modulus + `82` exponent (big-endian).
    /// ECDSA: `86` = uncompressed point `04 || X || Y`.
    fn pubkey_do(&self) -> Vec<u8> {
        match &self.key {
            KeyType::Rsa { n, e, key_size, .. } => {
                let mut modulus = n.to_bytes_be();
                if modulus.len() < *key_size {
                    let mut padded = vec![0u8; key_size - modulus.len()];
                    padded.extend_from_slice(&modulus);
                    modulus = padded;
                }
                let exp = e.to_bytes_be();

                let mut content = Vec::with_capacity(4 + modulus.len() + 2 + exp.len());
                content.extend_from_slice(&[0x81, 0x82, 0x01, 0x00]); // modulus tag + length
                content.extend_from_slice(&modulus);
                content.push(0x82); // exponent tag
                content.push(exp.len() as u8);
                content.extend_from_slice(&exp);

                let mut do_ = Vec::with_capacity(5 + content.len());
                do_.extend_from_slice(&[0x7F, 0x49, 0x82, 0x01, 0x09]); // 265-byte content
                do_.extend_from_slice(&content);
                do_
            }
            KeyType::Ec { point, .. } => {
                // content = 86 <point_len> <point>; point fits a single-byte length.
                let mut content = Vec::with_capacity(2 + point.len());
                content.push(0x86);
                content.push(point.len() as u8);
                content.extend_from_slice(point);

                // DO = 7F 49 <len> <content> (content <= 138 -> single-byte length).
                let mut do_ = Vec::with_capacity(3 + content.len());
                do_.extend_from_slice(&[0x7F, 0x49]);
                do_.push(content.len() as u8);
                do_.extend_from_slice(&content);
                do_
            }
        }
    }

    // ---------------------------------------------------------------------
    // RSA
    // ---------------------------------------------------------------------
    fn rsa_raw(n: &BigUint, d: &BigUint, key_size: usize, value: &[u8]) -> Vec<u8> {
        let v = BigUint::from_bytes_be(value);
        let result = v.modpow(d, n);
        let mut bytes = result.to_bytes_be();
        if bytes.len() < key_size {
            let mut padded = vec![0u8; key_size - bytes.len()];
            padded.extend_from_slice(&bytes);
            bytes = padded;
        }
        bytes
    }

    fn rsa_pkcs1_sign(
        n: &BigUint,
        d: &BigUint,
        key_size: usize,
        data: &[u8],
    ) -> Result<Vec<u8>, String> {
        if data.len() + 11 > key_size {
            return Err("Data too large".to_string());
        }
        let mut em = vec![0u8; key_size];
        em[0] = 0x00;
        em[1] = 0x01;
        let ps_len = key_size - data.len() - 3;
        em[2..2 + ps_len].fill(0xFF);
        em[2 + ps_len] = 0x00;
        em[3 + ps_len..].copy_from_slice(data);
        Ok(Self::rsa_raw(n, d, key_size, &em))
    }
}

/// Parse an unencrypted PKCS#8 PEM private key as an ECDSA signing key, trying
/// the supported curves in order. Returns `None` when the key is not an EC key
/// (RSA) or is encrypted.
fn parse_ec_signing_key_pem(pem: &str) -> Option<EcSigningKey> {
    if let Ok(k) = P256SigningKey::from_pkcs8_pem(pem) {
        return Some(EcSigningKey::P256(k));
    }
    if let Ok(k) = P384SigningKey::from_pkcs8_pem(pem) {
        return Some(EcSigningKey::P384(k));
    }
    // P-521: the p521 0.13 wrapper lacks a working DecodePrivateKey impl, so the
    // PKCS#8 is parsed manually and the scalar handed to `from_slice`.
    let scalar = parse_pkcs8_ec_private_scalar(pem)?;
    P521SigningKey::from_slice(&scalar).ok().map(EcSigningKey::P521)
}

/// Decrypt an encrypted PKCS#8 PEM EC key with the given password (the card PIN)
/// on the known curve.
fn decrypt_ec_signing_key_pem(curve: EcCurve, pem: &str, password: &[u8]) -> Option<EcSigningKey> {
    match curve {
        EcCurve::P256 => P256SigningKey::from_pkcs8_encrypted_pem(pem, password)
            .ok()
            .map(EcSigningKey::P256),
        EcCurve::P384 => P384SigningKey::from_pkcs8_encrypted_pem(pem, password)
            .ok()
            .map(EcSigningKey::P384),
        EcCurve::P521 => {
            // P-521: same manual path; the PKCS#8 is decrypted via pkcs8's PBES2
            // support and the resulting ECPrivateKey parsed by hand.
            let (_, doc) = pkcs8::SecretDocument::from_pem(pem).ok()?;
            let epki = pkcs8::EncryptedPrivateKeyInfo::try_from(doc.as_bytes()).ok()?;
            let decrypted = epki.decrypt(password).ok()?;
            let scalar = parse_pkcs8_ec_private_scalar_der(decrypted.as_bytes())?;
            P521SigningKey::from_slice(&scalar).ok().map(EcSigningKey::P521)
        }
    }
}

/// Parse the EC private scalar from an unencrypted PKCS#8 PEM.
fn parse_pkcs8_ec_private_scalar(pem: &str) -> Option<Vec<u8>> {
    let der = pem::parse(pem).ok()?.into_contents();
    parse_pkcs8_ec_private_scalar_der(&der)
}

/// Parse the EC private scalar from a PKCS#8 DER `privateKey` OCTET STRING
/// (i.e. the ECPrivateKey DER).
fn parse_pkcs8_ec_private_scalar_der(der: &[u8]) -> Option<Vec<u8>> {
    let (pki_start, pki_end) = der_sequence(der, 0)?;
    // version INTEGER
    let (_, after) = der_integer(der, pki_start)?;
    // AlgorithmIdentifier SEQUENCE
    let (_, alg_end) = der_sequence(der, pki_start + after)?;
    // privateKey OCTET STRING containing the ECPrivateKey DER
    let pos = alg_end;
    if der.get(pos) != Some(&0x04) {
        return None;
    }
    let (len, len_size) = read_ber_len(&der[pos + 1..])?;
    let ec_start = pos + 1 + len_size;
    let ec_end = ec_start + len;
    if ec_end > pki_end {
        return None;
    }
    parse_ec_private_scalar(&der[ec_start..ec_end])
}

/// Parse the ECPrivateKey DER (`SEQUENCE { INTEGER version, OCTET STRING
/// privateKey, ... }`) and return the private scalar bytes.
fn parse_ec_private_scalar(der: &[u8]) -> Option<Vec<u8>> {
    let (start, end) = der_sequence(der, 0)?;
    // version INTEGER
    let (_, after) = der_integer(der, start)?;
    // privateKey OCTET STRING
    let pos = start + after;
    if der.get(pos) != Some(&0x04) {
        return None;
    }
    let (len, len_size) = read_ber_len(&der[pos + 1..])?;
    let s_start = pos + 1 + len_size;
    let s_end = s_start + len;
    if s_end > end || s_end > der.len() {
        return None;
    }
    Some(der[s_start..s_end].to_vec())
}

/// Extract the public key from an X.509 certificate DER: walks the certificate
/// to the subjectPublicKeyInfo SEQUENCE and parses the algorithm + key.
/// Used when the private key is encrypted: the public part must be served (7F49)
/// before any PIN.
fn extract_spki_public(cert_der: &[u8]) -> Result<SpkiPublic, String> {
    const RSA_OID: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01];
    const EC_OID: &[u8] = &[0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x02, 0x01];

    // Walk the DER: certificate SEQUENCE -> tbsCertificate SEQUENCE, then scan its
    // top-level children (any TLV) for the subjectPublicKeyInfo SEQUENCE (the one
    // containing the rsaEncryption or ecPublicKey OID).
    let cert_seq = der_sequence(cert_der, 0).ok_or("cert: not a SEQUENCE")?;
    let tbs = der_sequence(cert_der, cert_seq.0).ok_or("cert: no tbsCertificate")?;

    let mut pos = tbs.0;
    while pos < tbs.1 {
        let tag = *cert_der.get(pos).ok_or("cert: truncated")?;
        let (len, len_size) = read_ber_len(&cert_der[pos + 1..]).ok_or("cert: bad length")?;
        let content_start = pos + 1 + len_size;
        let content_end = content_start + len;
        if content_end > tbs.1 {
            break;
        }
        if tag == 0x30 {
            let spki = &cert_der[content_start..content_end];
            if spki.windows(RSA_OID.len()).any(|w| w == RSA_OID) {
                return parse_spki_rsa_public(spki)
                    .map(|(n, e)| SpkiPublic::Rsa { n, e })
                    .ok_or_else(|| "cert: no RSA key in SPKI".to_string());
            }
            if spki.windows(EC_OID.len()).any(|w| w == EC_OID) {
                return parse_spki_ec_public(spki)
                    .map(|(curve, point)| SpkiPublic::Ec { curve, point })
                    .ok_or_else(|| "cert: no EC key in SPKI".to_string());
            }
        }
        pos = content_end;
    }
    Err("cert: no supported public key found".to_string())
}

/// Parse the subjectPublicKeyInfo content: algorithm SEQUENCE + BIT STRING
/// containing the RSAPublicKey DER (SEQUENCE of INTEGER n, INTEGER e).
fn parse_spki_rsa_public(spki: &[u8]) -> Option<(BigUint, BigUint)> {
    let (_, alg_end) = der_sequence(spki, 0)?;
    let bit_string = &spki[alg_end..];
    if bit_string.first() != Some(&0x03) {
        return None;
    }
    let (len, len_size) = read_ber_len(&bit_string[1..])?;
    let content = bit_string.get(1 + len_size..1 + len_size + len)?;
    parse_rsa_public_components(&content[1..]) // skip the unused-bits byte
}

/// Parse the subjectPublicKeyInfo content for an EC key: the BIT STRING contains
/// the uncompressed point `04 || X || Y`; the curve is derived from its length.
fn parse_spki_ec_public(spki: &[u8]) -> Option<(EcCurve, Vec<u8>)> {
    let (_, alg_end) = der_sequence(spki, 0)?;
    let bit_string = &spki[alg_end..];
    if bit_string.first() != Some(&0x03) {
        return None;
    }
    let (len, len_size) = read_ber_len(&bit_string[1..])?;
    let content = bit_string.get(1 + len_size..1 + len_size + len)?;
    let point = &content[1..]; // skip the unused-bits byte
    if point.first() != Some(&0x04) {
        return None;
    }
    let curve = match point.len() {
        65 => EcCurve::P256,
        97 => EcCurve::P384,
        133 => EcCurve::P521,
        _ => return None,
    };
    Some((curve, point.to_vec()))
}

/// Parse `SEQUENCE { INTEGER n, INTEGER e }` (PKCS#1 RSAPublicKey).
fn parse_rsa_public_components(der: &[u8]) -> Option<(BigUint, BigUint)> {
    let (start, _) = der_sequence(der, 0)?;
    let (n, n_after) = der_integer(der, start)?;
    let (e, _) = der_integer(der, start + n_after)?;
    Some((n, e))
}

fn der_integer(data: &[u8], offset: usize) -> Option<(BigUint, usize)> {
    if data.get(offset) != Some(&0x02) {
        return None;
    }
    let (len, len_size) = read_ber_len(&data[offset + 1..])?;
    let start = offset + 1 + len_size;
    let end = start + len;
    if end > data.len() {
        return None;
    }
    let value = if data[start] == 0 {
        BigUint::from_bytes_be(&data[start + 1..end])
    } else {
        BigUint::from_bytes_be(&data[start..end])
    };
    Some((value, end - offset))
}

/// Return the (content_start, content_end) of a DER SEQUENCE at `offset`.
fn der_sequence(data: &[u8], offset: usize) -> Option<(usize, usize)> {
    if data.get(offset) != Some(&0x30) {
        return None;
    }
    let (len, len_size) = read_ber_len(&data[offset + 1..])?;
    let start = offset + 1 + len_size;
    let end = start + len;
    if end > data.len() {
        return None;
    }
    Some((start, end))
}

/// Read a BER length (short or long form) returning (value, bytes_consumed).
fn read_ber_len(data: &[u8]) -> Option<(usize, usize)> {
    let first = *data.first()?;
    if first < 0x80 {
        return Some((first as usize, 1));
    }
    let num_bytes = (first & 0x7F) as usize;
    if num_bytes == 0 || num_bytes > 4 || data.len() < 1 + num_bytes {
        return None;
    }
    let mut len = 0usize;
    for i in 0..num_bytes {
        len = (len << 8) | data[1 + i] as usize;
    }
    Some((len, 1 + num_bytes))
}

/// Find a known GIDS 2-byte tag inside a GET DATA APDU data field.
fn find_gids_tag(data: &[u8]) -> Option<(u8, u8)> {
    const TAGS: [(u8, u8); 8] = [
        (0xDF, 0x1F),
        (0xDF, 0x20),
        (0xDF, 0x22),
        (0xDF, 0x23),
        (0xDF, 0x24),
        (0x7F, 0x73),
        (0x7F, 0x49),
        (0x2F, 0x01),
    ];
    TAGS.iter().copied().find(|&t| data.windows(2).any(|w| w == [t.0, t.1]))
}
