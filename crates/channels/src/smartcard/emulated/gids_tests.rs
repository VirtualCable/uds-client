// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! Tests for the GIDS emulation engine.

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use num_bigint::BigUint;
    use rsa::pkcs1::EncodeRsaPrivateKey;

    use crate::smartcard::emulated::gids_engine::{REFERENCE_CARDID, REFERENCE_GUID, GIDS_AID, GidsEngine};
    use crate::smartcard::emulated::helpers::{parse_apdu_header, extract_apdu_data};

    fn make_engine(pin: &str) -> (GidsEngine, BigUint, BigUint, BigUint) {
        let mut rng = rsa::rand_core::OsRng;
        let key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pkcs1 = key.to_pkcs1_der().unwrap();
        let (n, e, d) = crate::smartcard::emulated::helpers::parse_rsa_pkcs1_components(pkcs1.as_bytes()).unwrap();
        // Pseudo-random "certificate" so the zlib-compressed DF24 content stays large
        // (a repetitive cert would compress to a few bytes and skip 61 XX chaining).
        let mut x: u32 = 0x12345678;
        let mut cert = Vec::with_capacity(600);
        for _ in 0..600 {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            cert.push((x >> 16) as u8);
        }
        let engine = GidsEngine::new(cert, key, pin.to_string());
        (engine, n, e, d)
    }

    /// Read a full file through the engine (GET DATA + GET RESPONSE chaining).
    fn read_file(engine: &mut GidsEngine, p1: u8, p2: u8, tag: &[u8; 2]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut resp = engine.process_apdu(&get_data_apdu(p1, p2, tag));
        loop {
            let sw = status(&resp);
            out.extend_from_slice(data(&resp));
            if sw >> 8 != 0x61 {
                break;
            }
            let le = (sw & 0xFF) as u8;
            resp = engine.process_apdu(&get_response(le));
        }
        out
    }

    fn status(resp: &[u8]) -> u16 {
        u16::from_be_bytes([resp[resp.len() - 2], resp[resp.len() - 1]])
    }

    fn data(resp: &[u8]) -> &[u8] {
        &resp[..resp.len() - 2]
    }

    fn apdu_select_gids(p2: u8) -> Vec<u8> {
        let mut apdu = vec![0x00, 0xA4, 0x04, p2, 0x09];
        apdu.extend_from_slice(GIDS_AID);
        apdu.push(0x00);
        apdu
    }

    fn get_data_apdu(p1: u8, p2: u8, tag: &[u8; 2]) -> Vec<u8> {
        vec![0x00, 0xCB, p1, p2, 0x04, 0x5C, 0x02, tag[0], tag[1], 0x00]
    }

    fn get_data_7f49() -> Vec<u8> {
        vec![
            0x00, 0xCB, 0x3F, 0xFF, 0x0A, 0x70, 0x08, 0x84, 0x01, 0x81, 0xA5, 0x03, 0x7F, 0x49,
            0x80, 0x00,
        ]
    }

    fn get_response(le: u8) -> Vec<u8> {
        vec![0x00, 0xC0, 0x00, 0x00, le]
    }

    fn verify_apdu(pin: &str) -> Vec<u8> {
        let mut apdu = vec![0x00, 0x20, 0x00, 0x80, pin.len() as u8];
        apdu.extend_from_slice(pin.as_bytes());
        apdu
    }

    fn pso_sign_apdu() -> Vec<u8> {
        // SHA-256 DigestInfo (51 bytes) like the reference card's PSO.
        let digest_info: [u8; 51] = [
            0x30, 0x31, 0x30, 0x0D, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02,
            0x01, 0x05, 0x00, 0x04, 0x20, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99,
            0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
            0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00,
        ];
        let mut apdu = vec![0x00, 0x2A, 0x9E, 0x9A, digest_info.len() as u8];
        apdu.extend_from_slice(&digest_info);
        apdu
    }

    fn raw_rsa_public(value: &[u8], e: &BigUint, n: &BigUint) -> Vec<u8> {
        let v = BigUint::from_bytes_be(value);
        let r = v.modpow(e, n);
        let mut bytes = r.to_bytes_be();
        if bytes.len() < 256 {
            let mut pad = vec![0u8; 256 - bytes.len()];
            pad.extend_from_slice(&bytes);
            bytes = pad;
        }
        bytes
    }

    #[test]
    fn select_gids_aid_returns_fci() {
        let (mut engine, _, _, _) = make_engine("1234");
        let resp = engine.process_apdu(&apdu_select_gids(0x00));
        assert_eq!(status(&resp), 0x9000);
        assert_eq!(data(&resp), &[
            0x61, 0x12, 0x4F, 0x0B, 0xA0, 0x00, 0x00, 0x03, 0x97, 0x42, 0x54, 0x46, 0x59, 0x02,
            0x01, 0x73, 0x03, 0x40, 0x01, 0xC0,
        ]);
    }

    #[test]
    fn select_unknown_aid_returns_6a82() {
        let (mut engine, _, _, _) = make_engine("1234");
        let resp = engine.process_apdu(&[0x00, 0xA4, 0x04, 0x00, 0x09, 0xA0, 0x00, 0x00, 0x03, 0x08, 0x00, 0x00, 0x10, 0x00]);
        assert_eq!(status(&resp), 0x6A82);
    }

    #[test]
    fn get_data_cardid_matches_reference() {
        let (mut engine, _, _, _) = make_engine("1234");
        let resp = engine.process_apdu(&get_data_apdu(0xA0, 0x12, &[0xDF, 0x20]));
        assert_eq!(status(&resp), 0x9000);
        let d = data(&resp);
        assert_eq!(&d[..3], &[0xDF, 0x20, 0x10]);
        assert_eq!(&d[3..], &REFERENCE_CARDID);
    }

    #[test]
    fn get_data_cardapps_and_config() {
        let (mut engine, _, _, _) = make_engine("1234");
        let resp = engine.process_apdu(&get_data_apdu(0xA0, 0x00, &[0xDF, 0x1F]));
        assert_eq!(status(&resp), 0x9000);
        assert_eq!(data(&resp)[..3], [0xDF, 0x1F, 0x81]);

        let resp = engine.process_apdu(&get_data_apdu(0xA0, 0x10, &[0xDF, 0x22]));
        assert_eq!(status(&resp), 0x9000);
        assert_eq!(data(&resp), &[0xDF, 0x22, 0x06, 0x00, 0x00, 0x01, 0x00, 0x05, 0x00]);
    }

    #[test]
    fn get_data_cmapfile_contains_reference_guid() {
        let (mut engine, _, _, _) = make_engine("1234");
        let resp = engine.process_apdu(&get_data_apdu(0xA0, 0x10, &[0xDF, 0x23]));
        assert_eq!(status(&resp), 0x9000);
        let d = data(&resp);
        assert_eq!(&d[..3], &[0xDF, 0x23, 0x56]);
        let guid_utf16: Vec<u8> = REFERENCE_GUID.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(&d[3..3 + guid_utf16.len()], &guid_utf16);
    }

    #[test]
    fn get_data_7f73_is_not_found() {
        let (mut engine, _, _, _) = make_engine("1234");
        let resp = engine.process_apdu(&get_data_apdu(0x3F, 0xFF, &[0x7F, 0x73]));
        assert_eq!(status(&resp), 0x6A88);
    }

    #[test]
    fn get_data_7f49_returns_pubkey_with_chaining() {
        let (mut engine, n, e, _) = make_engine("1234");
        let resp = engine.process_apdu(&get_data_7f49());
        assert_eq!(status(&resp) >> 8, 0x61); // more data available
        assert_eq!(data(&resp).len(), 256);

        // First chunk must be the 7F49 DO header + start of modulus.
        let first = data(&resp);
        assert_eq!(&first[..5], &[0x7F, 0x49, 0x82, 0x01, 0x09]);
        assert_eq!(&first[5..9], &[0x81, 0x82, 0x01, 0x00]);

        // GET RESPONSE with Le=0x0E yields the remaining 14 bytes.
        let resp2 = engine.process_apdu(&get_response(0x0E));
        assert_eq!(status(&resp2), 0x9000);
        assert_eq!(data(&resp2).len(), 14);

        // Reassemble the full DO and check the modulus matches the key.
        let mut full = Vec::new();
        full.extend_from_slice(first);
        full.extend_from_slice(data(&resp2));
        let modulus = &full[9..9 + 256];
        let mut key_mod = n.to_bytes_be();
        if key_mod.len() < 256 {
            let mut pad = vec![0u8; 256 - key_mod.len()];
            pad.extend_from_slice(&key_mod);
            key_mod = pad;
        }
        assert_eq!(modulus, key_mod.as_slice());
        // exponent 01 00 01
        assert_eq!(&full[full.len() - 5..], &[0x82, 0x03, 0x01, 0x00, 0x01]);
        let _ = e;
    }

    #[test]
    fn get_data_df24_returns_compressed_cert_with_chaining() {
        let (mut engine, _, _, _) = make_engine("1234");
        let resp = engine.process_apdu(&get_data_apdu(0xA0, 0x10, &[0xDF, 0x24]));
        assert_eq!(status(&resp) >> 8, 0x61);
        assert_eq!(data(&resp)[..3], [0xDF, 0x24, 0x82]);
    }

    #[test]
    fn df24_content_is_01_00_len_zlib_compressed_cert() {
        use std::io::Read;
        let (mut engine, _, _, _) = make_engine("1234");
        let do_bytes = read_file(&mut engine, 0xA0, 0x10, &[0xDF, 0x24]);
        // DO = DF 24 <len> <content>; content = 01 00 <lenLE> <zlib cert>.
        assert_eq!(&do_bytes[..2], &[0xDF, 0x24]);
        let content = &do_bytes[5..]; // skip DF 24 + 3-byte BER length (82 xx xx)
        assert_eq!(&content[..2], &[0x01, 0x00]);
        let uncompressed_len = u16::from_le_bytes([content[2], content[3]]) as usize;
        assert!(uncompressed_len > 0);
        let compressed = &content[4..];
        let mut dec = flate2::read::ZlibDecoder::new(compressed);
        let mut cert = Vec::new();
        dec.read_to_end(&mut cert).unwrap();
        assert_eq!(cert.len(), uncompressed_len);
        assert_eq!(cert[0], (0x12345678_u32.wrapping_mul(1_103_515_245).wrapping_add(12_345) >> 16) as u8);
    }

    #[test]
    fn verify_wrong_then_correct_pin() {
        let (mut engine, _, _, _) = make_engine("1234");
        let resp = engine.process_apdu(&verify_apdu("9999"));
        assert_eq!(status(&resp) >> 8, 0x63); // 63 CX
        let resp = engine.process_apdu(&verify_apdu("1234"));
        assert_eq!(status(&resp), 0x9000);
    }

    #[test]
    fn mse_set_ok_and_pso_requires_pin() {
        let (mut engine, _, _, _) = make_engine("1234");
        let mse = [0x00, 0x22, 0x41, 0xB6, 0x06, 0x80, 0x01, 0x57, 0x84, 0x01, 0x81];
        assert_eq!(status(&engine.process_apdu(&mse)), 0x9000);

        // Sign without PIN → 6982.
        let resp = engine.process_apdu(&pso_sign_apdu());
        assert_eq!(status(&resp), 0x6982);
    }

    #[test]
    fn pso_sign_after_verify_produces_valid_signature() {
        let (mut engine, n, e, _) = make_engine("1234");
        engine.process_apdu(&verify_apdu("1234"));
        let resp = engine.process_apdu(&pso_sign_apdu());
        assert_eq!(status(&resp), 0x9000);
        let sig = data(&resp);
        assert_eq!(sig.len(), 256);

        // Raw RSA public verify: decrypt the signature and check PKCS#1 v1.5 EM
        // (00 01 FF..00 <digestinfo>) is recovered.
        let em = raw_rsa_public(sig, &e, &n);
        assert_eq!(&em[..2], &[0x00, 0x01]);
        let sep = em[2..].iter().position(|&b| b == 0x00).unwrap();
        assert!(sep >= 8);
        assert_eq!(em.len(), 256);
    }

    #[test]
    fn unknown_apdu_returns_6986() {
        let (mut engine, _, _, _) = make_engine("1234");
        let resp = engine.process_apdu(&[0x00, 0xCA, 0x00, 0x00, 0x00]);
        assert_eq!(status(&resp), 0x6986);
    }

    #[test]
    fn parse_get_data_7f49_apdu_tag() {
        // Sanity: the 7F49 APDU parses with our helpers (tag extraction path).
        let apdu = get_data_7f49();
        let h = parse_apdu_header(&apdu).unwrap();
        assert_eq!((h.ins, h.p1, h.p2), (0xCB, 0x3F, 0xFF));
        let (data, le) = extract_apdu_data(&apdu);
        assert!(data.windows(2).any(|w| w == [0x7F, 0x49]));
        assert_eq!(le, Some(256));
    }
}
