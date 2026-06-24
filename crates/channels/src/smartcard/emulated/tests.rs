// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Tests for the emulated smartcard backend.

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use num_bigint::BigUint;
    use num_traits::Zero;
    use rsa::RsaPrivateKey;
    use rsa::pkcs1::EncodeRsaPrivateKey;

    use crate::smartcard::emulated::consts::*;
    use crate::smartcard::emulated::engine::GidsEngine;
    use crate::smartcard::emulated::helpers::*;
    use crate::smartcard::emulated::types::*;

    fn make_test_engine() -> GidsEngine {
        let mut rng = rsa::rand_core::OsRng;
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let der = key.to_pkcs1_der().unwrap();
        let (n, d) = parse_rsa_pkcs1_components(der.as_bytes()).unwrap();
        let key_size = (n.bits() as usize).div_ceil(8);
        let cert_der = vec![0x30, 0x82, 0x01, 0x00];
        let vfs = VirtualFs::new(&cert_der, &n, &d, key_size);
        GidsEngine {
            cert_der,
            pin: "1234".into(),
            vfs,
            session: SessionState::new(),
            n,
            d,
            key_size,
        }
    }

    #[test]
    fn tlv_write_and_find() {
        let mut buf = Vec::new();
        tlv_write(&mut buf, 0xDF1F, &[1, 2, 3]);
        tlv_write(&mut buf, 0xDF20, &[4, 5]);
        assert_eq!(tlv_find(&buf, 0xDF1F), Some(&[1, 2, 3][..]));
        assert_eq!(tlv_find(&buf, 0xDF20), Some(&[4, 5][..]));
        assert_eq!(tlv_find(&buf, 0xDEAD), None);
    }
    #[test]
    fn tlv_write_single_byte_tag() {
        let mut b = vec![];
        tlv_write(&mut b, 0x81, &[0xFF]);
        assert_eq!(b, &[0x81, 0x01, 0xFF]);
    }
    #[test]
    fn tlv_write_long_length() {
        let mut b = vec![];
        tlv_write(&mut b, 0xDF24, &vec![0xAB; 300]);
        assert_eq!(tlv_find(&b, 0xDF24).map(|d| d.len()), Some(300));
    }
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
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let (n, d) = parse_rsa_pkcs1_components(key.to_pkcs1_der().unwrap().as_bytes()).unwrap();
        assert_eq!(n.to_bytes_be(), key.n().to_bytes_be());
        assert!(d > BigUint::zero());
    }

    #[test]
    fn select_by_aid_returns_fci() {
        let mut e = make_test_engine();
        let mut a = vec![0x00, 0xA4, 0x04, 0x00, GIDS_AID.len() as u8];
        a.extend_from_slice(&GIDS_AID);
        let r = e.process_apdu(&a);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_SUCCESS
        );
        assert_eq!(&r[..r.len() - 2], &GIDS_FCI);
    }
    #[test]
    fn select_wrong_aid_returns_not_found() {
        let mut e = make_test_engine();
        let r = e.process_apdu(&[0x00, 0xA4, 0x04, 0x00, 0x03, 0x01, 0x02, 0x03]);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_FILE_NOT_FOUND
        );
    }
    #[test]
    fn verify_correct_pin() {
        let mut e = make_test_engine();
        let r = e.process_apdu(&[0x00, 0x20, 0x00, 0x80, 0x04, b'1', b'2', b'3', b'4']);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_SUCCESS
        );
        assert!(e.session.pin_verified);
    }
    #[test]
    fn verify_wrong_pin() {
        let mut e = make_test_engine();
        let r = e.process_apdu(&[0x00, 0x20, 0x00, 0x80, 0x04, b'x', b'x', b'x', b'x']);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_VERIFY_FAILED | 2
        );
    }
    #[test]
    fn verify_blocks_after_3_failures() {
        let mut e = make_test_engine();
        for i in (0..3).rev() {
            let r = e.process_apdu(&[0x00, 0x20, 0x00, 0x80, 0x04, b'x', b'x', b'x', b'x']);
            assert_eq!(
                u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
                SW_VERIFY_FAILED | i as u16
            );
        }
        let r = e.process_apdu(&[0x00, 0x20, 0x00, 0x80, 0x04, b'x', b'x', b'x', b'x']);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_AUTH_METHOD_BLOCKED
        );
    }
    #[test]
    fn get_data_cardcf() {
        let mut e = make_test_engine();
        let mut s = vec![0x00, 0xA4, 0x04, 0x00, GIDS_AID.len() as u8];
        s.extend_from_slice(&GIDS_AID);
        e.process_apdu(&s);
        let r = e.process_apdu(&[0x00, 0xCB, 0x3F, 0xFF, 0x04, 0x5C, 0x02, 0xDF, 0x22]);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_SUCCESS
        );
        assert_eq!(&r[..r.len() - 2], &CARDCF_CONTENT);
    }
    #[test]
    fn get_data_nonexistent_do() {
        let mut e = make_test_engine();
        let mut s = vec![0x00, 0xA4, 0x04, 0x00, GIDS_AID.len() as u8];
        s.extend_from_slice(&GIDS_AID);
        e.process_apdu(&s);
        let r = e.process_apdu(&[0x00, 0xCB, 0x3F, 0xFF, 0x04, 0x5C, 0x02, 0xDE, 0xAD]);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_REF_DATA_NOT_FOUND
        );
    }
    #[test]
    fn mse_set_sign() {
        let mut e = make_test_engine();
        let r = e.process_apdu(&[
            0x00, 0x22, 0x41, 0xB6, 0x06, 0x80, 0x01, 0x47, 0x84, 0x01, 0x81,
        ]);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_SUCCESS
        );
        assert!(e.session.current_se.unwrap().crt == CRT_SIGN);
    }
    #[test]
    fn mse_set_decrypt() {
        let mut e = make_test_engine();
        e.process_apdu(&[
            0x00, 0x22, 0x41, 0xB8, 0x06, 0x80, 0x01, 0x47, 0x84, 0x01, 0x81,
        ]);
        assert_eq!(e.session.current_se.unwrap().crt, CRT_CONF);
    }
    #[test]
    fn pso_sign_without_pin_fails() {
        let mut e = make_test_engine();
        e.process_apdu(&[
            0x00, 0x22, 0x41, 0xB6, 0x06, 0x80, 0x01, 0x47, 0x84, 0x01, 0x81,
        ]);
        let r = e.process_apdu(&[0x00, 0x2A, 0x9E, 0x9A, 0x02, 0xAB, 0xCD]);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_SECURITY_NOT_SATISFIED
        );
    }
    #[test]
    fn pso_sign_with_pin_succeeds() {
        let mut e = make_test_engine();
        e.process_apdu(&make_select());
        e.process_apdu(&[0x00, 0x20, 0x00, 0x80, 0x04, b'1', b'2', b'3', b'4']);
        e.process_apdu(&[
            0x00, 0x22, 0x41, 0xB6, 0x06, 0x80, 0x01, 0x47, 0x84, 0x01, 0x81,
        ]);
        let di = vec![0x30, 0x31, 0x30, 0x0D, 0x06, 0x09, 0x60, 0x86, 0x48];
        let mut a = vec![0x00, 0x2A, 0x9E, 0x9A, di.len() as u8];
        a.extend_from_slice(&di);
        let r = e.process_apdu(&a);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_SUCCESS
        );
        assert_eq!(r.len() - 2, 256);
    }
    #[test]
    fn pso_decrypt_pkcs1() {
        let mut rng = rsa::rand_core::OsRng;
        let key = RsaPrivateKey::new(&mut rng, 1024).unwrap();
        let (n, d) = parse_rsa_pkcs1_components(key.to_pkcs1_der().unwrap().as_bytes()).unwrap();
        let ks = (n.bits() as usize).div_ceil(8);
        let mut e = GidsEngine {
            cert_der: vec![0xAA; 50],
            pin: "1234".into(),
            vfs: VirtualFs::new(&[0xAA; 50], &n, &d, ks),
            session: SessionState::new(),
            n,
            d,
            key_size: ks,
        };
        e.process_apdu(&make_select());
        e.process_apdu(&[0x00, 0x20, 0x00, 0x80, 0x04, b'1', b'2', b'3', b'4']);
        e.process_apdu(&[
            0x00, 0x22, 0x41, 0xB8, 0x06, 0x80, 0x01, 0x46, 0x84, 0x01, 0x81,
        ]);
        let pt = b"Hello!";
        let n_big = e.n.clone();
        let e_big = BigUint::from(65537u32);
        let padded = pkcs1v15_encrypt(pt, &n_big, &e_big, ks);
        let mut a = vec![0x00, 0x2A, 0x86, 0x80, padded.len() as u8];
        a.extend_from_slice(&padded);
        let r = e.process_apdu(&a);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_SUCCESS
        );
        assert_eq!(&r[..r.len() - 2], pt);
    }
    #[test]
    fn full_gids_flow() {
        let mut e = make_test_engine();
        let r = e.process_apdu(&make_select());
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_SUCCESS
        );
        let r = e.process_apdu(&[0x00, 0x20, 0x00, 0x80, 0x04, b'1', b'2', b'3', b'4']);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_SUCCESS
        );
        e.process_apdu(&[
            0x00, 0x22, 0x41, 0xB6, 0x06, 0x80, 0x01, 0x47, 0x84, 0x01, 0x81,
        ]);
        let di: Vec<u8> = vec![0x30, 0x31, 0x30, 0x0D, 0x06, 0x09, 0x60, 0x86, 0x48];
        let mut a = vec![0x00, 0x2A, 0x9E, 0x9A, di.len() as u8];
        a.extend_from_slice(&di);
        let r = e.process_apdu(&a);
        let sig = &r[..r.len() - 2];
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_SUCCESS
        );
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
    #[test]
    fn vfs_builds_all_dos() {
        let mut rng = rsa::rand_core::OsRng;
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let (n, d) = parse_rsa_pkcs1_components(key.to_pkcs1_der().unwrap().as_bytes()).unwrap();
        let ks = (n.bits() as usize).div_ceil(8);
        let vfs = VirtualFs::new(&[0xAA; 100], &n, &d, ks);
        assert!(vfs.read_do(EFID_MASTER, DO_FILESYSTEM_TABLE).is_some());
        assert!(vfs.read_do(EFID_MASTER, DO_KEYMAP).is_some());
        assert!(vfs.read_do(EFID_CARDID, DO_CARDID).is_some());
        assert!(vfs.read_do(EFID_COMMON, DO_CARDAPPS).is_some());
        assert!(vfs.read_do(EFID_COMMON, DO_CARDCF).is_some());
        assert!(vfs.read_do(EFID_COMMON, DO_CMAPFILE).is_some());
        assert!(vfs.read_do(EFID_COMMON, DO_KXC00).is_some());
    }
    #[test]
    fn vfs_kxc00_roundtrip() {
        use std::io::Read;
        let mut rng = rsa::rand_core::OsRng;
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let (n, d) = parse_rsa_pkcs1_components(key.to_pkcs1_der().unwrap().as_bytes()).unwrap();
        let ks = (n.bits() as usize).div_ceil(8);
        let cert = vec![0xAA; 100];
        let vfs = VirtualFs::new(&cert, &n, &d, ks);
        let k = vfs.read_do(EFID_COMMON, DO_KXC00).unwrap();
        assert_eq!(u16::from_le_bytes([k[0], k[1]]), 1);
        assert_eq!(u16::from_le_bytes([k[2], k[3]]) as usize, cert.len());
        let mut dec = flate2::read::ZlibDecoder::new(&k[4..]);
        let mut out = Vec::new();
        dec.read_to_end(&mut out).unwrap();
        assert_eq!(out, cert);
    }
    #[test]
    fn unknown_ins() {
        let r = make_test_engine().process_apdu(&[0x00, 0xFF, 0x00, 0x00]);
        assert_eq!(
            u16::from_be_bytes([r[r.len() - 2], r[r.len() - 1]]),
            SW_COMMAND_NOT_ALLOWED
        );
    }

    fn make_select() -> Vec<u8> {
        let mut a = vec![0x00, 0xA4, 0x04, 0x00, GIDS_AID.len() as u8];
        a.extend_from_slice(&GIDS_AID);
        a
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
