// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Tests for the emulated smartcard backend.

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use num_bigint::BigUint;
    use rsa::pkcs1::EncodeRsaPrivateKey;

    use crate::smartcard::emulated::consts::*;
    use crate::smartcard::emulated::euds_engine::EudsEngine;
    use crate::smartcard::emulated::euds_types::PinMode;
    use crate::smartcard::emulated::helpers::*;

    fn make_test_engine(pin_mode: PinMode) -> EudsEngine {
        let mut rng = rsa::rand_core::OsRng;
        let key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let cert_der = vec![0x30, 0x82, 0x01, 0x00];
        let pin = if pin_mode == PinMode::Required {
            "testpin".to_string()
        } else {
            String::new()
        };
        EudsEngine::new(cert_der, key, pin, pin_mode)
    }

    // -----------------------------------------------------------------
    // Response helpers
    // -----------------------------------------------------------------

    #[test]
    fn make_response_appends_status() {
        assert_eq!(make_response(&[1, 2], 0x9000), vec![1, 2, 0x90, 0x00]);
    }

    #[test]
    fn make_status_is_empty_data() {
        assert_eq!(make_status(0x6982), vec![0x69, 0x82]);
    }

    #[test]
    fn parse_rsa_components_roundtrip() {
        use rsa::traits::PublicKeyParts;
        let mut rng = rsa::rand_core::OsRng;
        let key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let (n, e, d) =
            parse_rsa_pkcs1_components(key.to_pkcs1_der().unwrap().as_bytes()).unwrap();
        assert_eq!(n.to_bytes_be(), key.n().to_bytes_be());
        assert_eq!(e, BigUint::from(65537u32));
        assert!(d > BigUint::from(0u32));
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
    }

    // -----------------------------------------------------------------
    // Extended APDU parsing
    // -----------------------------------------------------------------

    #[test]
    fn extract_apdu_case2_short() {
        let apdu = vec![0x80, 0xC0, 0x00, 0x00, 44];
        let (data, le) = extract_apdu_data(&apdu);
        assert!(data.is_empty());
        assert_eq!(le, Some(44));
    }

    #[test]
    fn extract_apdu_case3_short() {
        let apdu = vec![0x80, 0xB1, 0x00, 0x80, 0x04, 0x31, 0x32, 0x33, 0x34];
        let (data, le) = extract_apdu_data(&apdu);
        assert_eq!(data, &[0x31, 0x32, 0x33, 0x34]);
        assert_eq!(le, None);
    }

    #[test]
    fn extract_apdu_case4_short() {
        let mut apdu = vec![0x00, 0xA4, 0x04, 0x00, 0x09];
        apdu.extend_from_slice(EUDS_AID);
        apdu.push(0x00);
        let (data, le) = extract_apdu_data(&apdu);
        assert_eq!(data, EUDS_AID);
        assert_eq!(le, Some(256));
    }

    #[test]
    fn extract_apdu_case2_extended() {
        let apdu = vec![0x80, 0x46, 0x00, 0x00, 0x00, 0x01, 0x07];
        let (data, le) = extract_apdu_data(&apdu);
        assert!(data.is_empty());
        assert_eq!(le, Some(263));
    }

    #[test]
    fn extract_apdu_case4_extended() {
        let mut apdu = vec![0x80, 0xB3, 0x80, 0x86, 0x00, 0x01, 0x00];
        apdu.extend_from_slice(&[0xAB; 256]);
        apdu.extend_from_slice(&[0x00, 0x00]);
        let (data, le) = extract_apdu_data(&apdu);
        assert_eq!(data.len(), 256);
        assert_eq!(le, Some(0));
    }

    // -----------------------------------------------------------------
    // eUDS Engine: SELECT
    // -----------------------------------------------------------------

    #[test]
    fn select_euds_applet() {
        let mut e = make_test_engine(PinMode::NotRequired);
        let mut apdu = vec![0x00, 0xA4, 0x04, 0x00, EUDS_AID.len() as u8];
        apdu.extend_from_slice(EUDS_AID);
        apdu.push(0x00);
        let r = e.process_apdu(&apdu);
        assert_sw(&r, SW_SUCCESS);
    }

    #[test]
    fn select_wrong_aid() {
        let mut e = make_test_engine(PinMode::NotRequired);
        let apdu = vec![0x00, 0xA4, 0x04, 0x00, 0x05, 0x00, 0x01, 0x02, 0x03, 0x04, 0x00];
        let r = e.process_apdu(&apdu);
        assert_sw(&r, SW_FILE_NOT_FOUND);
    }

    // -----------------------------------------------------------------
    // eUDS Engine: VERIFY PIN
    // -----------------------------------------------------------------

    #[test]
    fn verify_pin_not_required() {
        let mut e = make_test_engine(PinMode::NotRequired);
        let apdu = vec![0x80, 0xB1, 0x00, 0x80, 0x04, b'1', b'2', b'3', b'4'];
        let r = e.process_apdu(&apdu);
        assert_sw(&r, SW_SUCCESS);
    }

    #[test]
    fn verify_pin_correct() {
        let mut e = make_test_engine(PinMode::Required);
        let apdu = vec![0x80, 0xB1, 0x00, 0x80, 0x07, 0x74, 0x65, 0x73, 0x74, 0x70, 0x69, 0x6E];
        let r = e.process_apdu(&apdu);
        assert_sw(&r, SW_SUCCESS);
        assert!(e.session.pin_verified);
    }

    #[test]
    fn verify_pin_wrong() {
        let mut e = make_test_engine(PinMode::Required);
        let apdu = vec![0x80, 0xB1, 0x00, 0x80, 0x05, 0x77, 0x72, 0x6F, 0x6E, 0x67];
        let r = e.process_apdu(&apdu);
        assert_eq!(r[0], 0x63);
        assert_eq!(r[1], 0xC2);
    }

    #[test]
    fn verify_pin_blocks_after_3_failures() {
        let mut e = make_test_engine(PinMode::Required);
        for i in (0..3).rev() {
            let apdu = vec![0x80, 0xB1, 0x00, 0x80, 0x05, 0x77, 0x72, 0x6F, 0x6E, 0x67];
            let r = e.process_apdu(&apdu);
            assert_eq!(r[0], 0x63);
            assert_eq!(r[1], 0xC0 | i);
        }
        let apdu = vec![0x80, 0xB1, 0x00, 0x80, 0x05, 0x77, 0x72, 0x6F, 0x6E, 0x67];
        let r = e.process_apdu(&apdu);
        assert_sw(&r, SW_AUTH_METHOD_BLOCKED);
    }

    // -----------------------------------------------------------------
    // eUDS Engine: GET CERTIFICATE
    // -----------------------------------------------------------------

    #[test]
    fn get_certificate_small() {
        let mut e = make_test_engine(PinMode::NotRequired);
        let apdu = vec![0x80, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x00];
        let r = e.process_apdu(&apdu);
        assert!(r.ends_with(&[0x90, 0x00]));
        assert_eq!(r.len() - 2, 4);
    }

    #[test]
    fn get_certificate_chaining() {
        let mut rng = rsa::rand_core::OsRng;
        let key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let large_cert = vec![0x42; 300];
        let mut e = EudsEngine::new(large_cert, key, String::new(), PinMode::NotRequired);

        let apdu = vec![0x80, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x00];
        let r = e.process_apdu(&apdu);
        assert_eq!(r.len(), 256 + 2);
        assert_eq!(r[256], 0x61);
        assert_eq!(r[257], 44);

        let get_resp = vec![0x80, 0xC0, 0x00, 0x00, 44];
        let r2 = e.process_apdu(&get_resp);
        assert_eq!(r2.len(), 44 + 2);
        assert!(r2.ends_with(&[0x90, 0x00]));
    }

    // -----------------------------------------------------------------
    // eUDS Engine: GET PUBLIC KEY
    // -----------------------------------------------------------------

    #[test]
    fn get_public_key_chaining() {
        let mut e = make_test_engine(PinMode::NotRequired);
        let apdu = vec![0x80, 0x46, 0x00, 0x00, 0x00, 0x01, 0x07];
        let r = e.process_apdu(&apdu);
        assert_eq!(r.len(), 256 + 2);
        assert_eq!(r[256], 0x61);
        assert_eq!(r[257], 0x07);

        let get_resp = vec![0x80, 0xC0, 0x00, 0x00, 0x07];
        let r2 = e.process_apdu(&get_resp);
        assert_eq!(r2.len(), 7 + 2);
        assert!(r2.ends_with(&[0x90, 0x00]));
    }

    #[test]
    fn get_public_key_format() {
        let mut e = make_test_engine(PinMode::NotRequired);
        let apdu = vec![0x80, 0x46, 0x00, 0x00, 0x00, 0x01, 0x07];
        let r = e.process_apdu(&apdu);
        let chunk1 = &r[..256];

        let get_resp = vec![0x80, 0xC0, 0x00, 0x00, 0x07];
        let r2 = e.process_apdu(&get_resp);
        let chunk2 = &r2[..7];

        let mut full = Vec::new();
        full.extend_from_slice(chunk1);
        full.extend_from_slice(chunk2);

        let exp_len =
            u16::from_be_bytes([full[0], full[1]]) as usize;
        assert_eq!(exp_len, 3);
        let exp_bytes = &full[2..2 + exp_len];
        assert_eq!(exp_bytes, &[0x01, 0x00, 0x01]);

        let mod_offset = 2 + exp_len;
        let mod_len =
            u16::from_be_bytes([full[mod_offset], full[mod_offset + 1]]) as usize;
        assert_eq!(mod_len, 256);
    }

    // -----------------------------------------------------------------
    // eUDS Engine: SIGN
    // -----------------------------------------------------------------

    #[test]
    fn sign_without_pin_fails() {
        let mut e = make_test_engine(PinMode::Required);
        let apdu = vec![0x80, 0xB2, 0x9E, 0x9A, 0x01, 0x00, 0x00];
        let r = e.process_apdu(&apdu);
        assert_sw(&r, SW_SECURITY_STATUS_NOT_SATISFIED);
    }

    #[test]
    fn sign_with_pin() {
        let mut e = make_test_engine(PinMode::Required);
        let verify = vec![0x80, 0xB1, 0x00, 0x80, 0x07, 0x74, 0x65, 0x73, 0x74, 0x70, 0x69, 0x6E];
        e.process_apdu(&verify);
        let di = vec![0x30, 0x31, 0x30, 0x0D, 0x06, 0x09, 0x60, 0x86, 0x48, 0x31, 0x02, 0x01,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut sign = vec![0x80, 0xB2, 0x9E, 0x9A, di.len() as u8];
        sign.extend_from_slice(&di);
        sign.push(0x00);
        let r = e.process_apdu(&sign);
        assert_sw(&r, SW_SUCCESS);
        assert_eq!(r.len(), 256 + 2);
    }

    #[test]
    fn sign_no_pin_mode() {
        let mut e = make_test_engine(PinMode::NotRequired);
        let di = vec![0x30, 0x31, 0x30, 0x0D, 0x06, 0x09, 0x60, 0x86, 0x48, 0x31, 0x02, 0x01,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut sign = vec![0x80, 0xB2, 0x9E, 0x9A, di.len() as u8];
        sign.extend_from_slice(&di);
        sign.push(0x00);
        let r = e.process_apdu(&sign);
        assert_sw(&r, SW_SUCCESS);
        assert_eq!(r.len(), 256 + 2);
    }

    #[test]
    fn full_sign_verify_pkcs1() {
        let mut e = make_test_engine(PinMode::NotRequired);
        let di: Vec<u8> = vec![0x30, 0x31, 0x30, 0x0D, 0x06, 0x09, 0x60, 0x86, 0x48, 0x31,
            0x02, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut sign = vec![0x80, 0xB2, 0x9E, 0x9A, di.len() as u8];
        sign.extend_from_slice(&di);
        sign.push(0x00);
        let r = e.process_apdu(&sign);
        assert_sw(&r, SW_SUCCESS);
        let sig = &r[..r.len() - 2];
        assert_eq!(sig.len(), 256);
        let n = e.n.clone();
        let e_big = BigUint::from(65537u32);
        let dec = BigUint::from_bytes_be(sig).modpow(&e_big, &n);
        let mut db = dec.to_bytes_be();
        if db.len() < 256 {
            let mut p = vec![0u8; 256 - db.len()];
            p.extend_from_slice(&db);
            db = p;
        }
        assert_eq!(db[0], 0x00);
        assert_eq!(db[1], 0x01);
        assert_eq!(&db[db.len() - di.len()..], &di[..]);
    }

    // -----------------------------------------------------------------
    // eUDS Engine: DECRYPT
    // -----------------------------------------------------------------

    #[test]
    fn decrypt_pkcs1() {
        let mut rng = rsa::rand_core::OsRng;
        let key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let mut e = EudsEngine::new(
            vec![0x30, 0x82],
            key.clone(),
            "testpin".to_string(),
            PinMode::Required,
        );
        let verify = vec![0x80, 0xB1, 0x00, 0x80, 0x07, 0x74, 0x65, 0x73, 0x74, 0x70, 0x69, 0x6E];
        e.process_apdu(&verify);

        let pt = b"eUDS test payload!";
        let (n, e_big, _) =
            parse_rsa_pkcs1_components(key.to_pkcs1_der().unwrap().as_bytes()).unwrap();
        let ks = (n.bits() as usize).div_ceil(8);
        let ciphertext = pkcs1v15_encrypt(pt, &n, &e_big, ks);
        assert_eq!(ciphertext.len(), 256);

        let mut decrypt = vec![0x80, 0xB3, 0x80, 0x86, 0x00, 0x01, 0x00];
        decrypt.extend_from_slice(&ciphertext);
        decrypt.extend_from_slice(&[0x00, 0x00]);
        let r = e.process_apdu(&decrypt);
        assert_sw(&r, SW_SUCCESS);
        let decrypted = &r[..r.len() - 2];
        assert_eq!(decrypted, pt);
    }

    #[test]
    fn decrypt_without_pin_fails() {
        let mut e = make_test_engine(PinMode::Required);
        let mut decrypt = vec![0x80, 0xB3, 0x80, 0x86, 0x00, 0x01, 0x00];
        decrypt.extend_from_slice(&[0xAB; 256]);
        decrypt.extend_from_slice(&[0x00, 0x00]);
        let r = e.process_apdu(&decrypt);
        assert_sw(&r, SW_SECURITY_STATUS_NOT_SATISFIED);
    }

    // -----------------------------------------------------------------
    // eUDS Engine: Chaining edge cases
    // -----------------------------------------------------------------

    #[test]
    fn get_response_without_chaining_returns_error() {
        let mut e = make_test_engine(PinMode::NotRequired);
        let apdu = vec![0x80, 0xC0, 0x00, 0x00, 0x10];
        let r = e.process_apdu(&apdu);
        assert_sw(&r, SW_WRONG_LC);
    }

    #[test]
    fn new_command_clears_chaining_buffer() {
        let mut rng = rsa::rand_core::OsRng;
        let key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let large_cert = vec![0x42; 300];
        let mut e = EudsEngine::new(large_cert, key, String::new(), PinMode::NotRequired);

        let apdu = vec![0x80, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x00];
        let _ = e.process_apdu(&apdu);
        assert!(e.session.chaining_buffer.is_some());

        let select = vec![0x00, 0xA4, 0x04, 0x00, 0x05, 0xDE, 0xAD, 0xBE, 0xEF, 0x00];
        let _ = e.process_apdu(&select);
        assert!(e.session.chaining_buffer.is_none());
    }

    // -----------------------------------------------------------------
    // eUDS Engine: Unknown INS
    // -----------------------------------------------------------------

    #[test]
    fn unknown_ins() {
        let mut e = make_test_engine(PinMode::NotRequired);
        let r = e.process_apdu(&[0x00, 0xFF, 0x00, 0x00]);
        assert_sw(&r, SW_COMMAND_NOT_ALLOWED);
    }

    // -----------------------------------------------------------------
    // ATR constant
    // -----------------------------------------------------------------

    #[test]
    fn atr_correct() {
        assert_eq!(EUDS_ATR, &[
            0x3B, 0x89, 0x01, 0x45, 0x55, 0x44, 0x53, 0x2D, 0x43, 0x61, 0x72, 0x64, 0x96
        ]);
    }

    // -----------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------

    fn assert_sw(response: &[u8], expected: u16) {
        let sw = u16::from_be_bytes([response[response.len() - 2], response[response.len() - 1]]);
        assert_eq!(sw, expected, "Expected SW {:04X}, got {:04X}", expected, sw);
    }

    fn pkcs1v15_encrypt(pt: &[u8], n: &BigUint, e: &BigUint, ks: usize) -> Vec<u8> {
        let mut em = vec![0u8; ks];
        em[0] = 0x00;
        em[1] = 0x02;
        for i in 0..(ks - pt.len() - 3) {
            em[2 + i] = (i as u8 % 254) + 1;
        }
        em[2 + (ks - pt.len() - 3)] = 0x00;
        em[3 + (ks - pt.len() - 3)..].copy_from_slice(pt);
        BigUint::from_bytes_be(&em).modpow(e, n).to_bytes_be()
    }
}
