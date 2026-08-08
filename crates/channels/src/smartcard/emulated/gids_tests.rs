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
    use rsa::pkcs8::{EncodePrivateKey, LineEnding};

    use crate::smartcard::emulated::gids_engine::{REFERENCE_CARDID, REFERENCE_GUID, GIDS_AID, GidsEngine};
    use crate::smartcard::emulated::helpers::{parse_apdu_header, extract_apdu_data, parse_rsa_pkcs1_components};

    fn make_engine() -> (GidsEngine, BigUint, BigUint, BigUint) {
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
        let key_pem = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let engine = GidsEngine::new(cert, key_pem).unwrap();
        (engine, n, e, d)
    }

    /// Engine with an ENCRYPTED private key (PIN = the key password). The certificate
    /// is built as a minimal but valid X.509 DER containing the RSA public key, since
    /// the encrypted path extracts the public part from the certificate.
    fn make_encrypted_engine(password: &str) -> GidsEngine {
        let mut rng = rsa::rand_core::OsRng;
        let key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pkcs1 = key.to_pkcs1_der().unwrap();
        let (n, e, _) = parse_rsa_pkcs1_components(pkcs1.as_bytes()).unwrap();
        let key_pem = key
            .to_pkcs8_encrypted_pem(&mut rng, password, LineEnding::LF)
            .unwrap()
            .to_string();
        GidsEngine::new(build_min_cert_der(&n, &e), key_pem).unwrap()
    }

    fn der_len(n: usize) -> Vec<u8> {
        if n < 0x80 {
            vec![n as u8]
        } else if n < 0x100 {
            vec![0x81, n as u8]
        } else {
            vec![0x82, (n >> 8) as u8, (n & 0xFF) as u8]
        }
    }

    fn der_seq(content: &[u8]) -> Vec<u8> {
        let mut v = vec![0x30];
        v.extend_from_slice(&der_len(content.len()));
        v.extend_from_slice(content);
        v
    }

    fn der_int(bi: &BigUint) -> Vec<u8> {
        let mut b = bi.to_bytes_be();
        if b.first() == Some(&0) {
            b.remove(0);
        }
        if b.first().map_or(true, |&x| x & 0x80 != 0) {
            b.insert(0, 0);
        }
        let mut v = vec![0x02];
        v.extend_from_slice(&der_len(b.len()));
        v.extend_from_slice(&b);
        v
    }

    /// Minimal X.509 certificate DER mimicking a real cert: SEQUENCE { SEQUENCE {
    /// [0] version (A0), INTEGER serial, SPKI { algorithm (rsaEncryption),
    /// BIT STRING (RSAPublicKey { n, e }) } } } — the engine's SPKI walk must skip
    /// the non-SEQUENCE tbs children (A0/02) to reach the SPKI.
    fn build_min_cert_der(n: &BigUint, e: &BigUint) -> Vec<u8> {
        let rsa_pub = der_seq(&[der_int(n), der_int(e)].concat());
        let mut bit_string = vec![0x03];
        bit_string.extend_from_slice(&der_len(rsa_pub.len() + 1));
        bit_string.push(0x00);
        bit_string.extend_from_slice(&rsa_pub);
        let mut alg = vec![0x06, 0x09];
        alg.extend_from_slice(&[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01]);
        alg.extend_from_slice(&[0x05, 0x00]);
        let spki = der_seq(&[der_seq(&alg), bit_string].concat());
        let version = vec![0xA0, 0x03, 0x02, 0x01, 0x02];
        let serial = der_int(&BigUint::from(0x1122_3344u64));
        let tbs = der_seq(&[version, serial, spki].concat());
        der_seq(&tbs)
    }

    // Curve OIDs (id-ecPublicKey params) for the EC test certificates.
    const EC_P256_OID: &[u8] = &[0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07];
    const EC_P384_OID: &[u8] = &[0x2B, 0x81, 0x04, 0x00, 0x22];
    const EC_P521_OID: &[u8] = &[0x2B, 0x81, 0x04, 0x00, 0x23];

    fn der_oid(oid: &[u8]) -> Vec<u8> {
        let mut v = vec![0x06, oid.len() as u8];
        v.extend_from_slice(oid);
        v
    }

    /// Minimal X.509 certificate with an EC subjectPublicKeyInfo: algorithm
    /// `id-ecPublicKey` + curve OID, BIT STRING = uncompressed point `04||X||Y`.
    fn build_min_ec_cert_der(point: &[u8], curve_oid: &[u8]) -> Vec<u8> {
        let ec_oid: [u8; 7] = [0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x02, 0x01];
        let alg = der_seq(&[der_oid(&ec_oid), der_oid(curve_oid)].concat());
        let mut bit_string = vec![0x03];
        bit_string.extend_from_slice(&der_len(point.len() + 1));
        bit_string.push(0x00);
        bit_string.extend_from_slice(point);
        let spki = der_seq(&[alg, bit_string].concat());
        let version = vec![0xA0, 0x03, 0x02, 0x01, 0x02];
        let serial = der_int(&BigUint::from(0x1122_3344u64));
        let tbs = der_seq(&[version, serial, spki].concat());
        der_seq(&tbs)
    }

    /// Assert the engine serves the EC public key in the `7F49` DO (tag 86, point)
    /// and that PSO produces a DER `SEQUENCE { r, s }` verifiable off-card.
    fn assert_ec_engine(engine: &mut GidsEngine, curve_oid: &[u8], hash: &[u8], der: &[u8]) {
        use p256::ecdsa::signature::hazmat::PrehashVerifier;

        let resp = engine.process_apdu(&get_data_7f49());
        assert_eq!(status(&resp), 0x9000); // EC point fits a single response
        let do_bytes = data(&resp);
        assert_eq!(&do_bytes[..2], &[0x7F, 0x49]);
        assert_eq!(do_bytes[3], 0x86);
        assert_eq!(&do_bytes[5..], der);

        let resp = engine.process_apdu(&pso_sign_hash_apdu(hash));
        assert_eq!(status(&resp), 0x9000);
        // The response must be a DER SEQUENCE; parse and verify with each curve.
        match curve_oid {
            o if o == EC_P256_OID => {
                let sig = p256::ecdsa::Signature::from_der(data(&resp)).unwrap();
                let vk = p256::ecdsa::VerifyingKey::from_sec1_bytes(der).unwrap();
                vk.verify_prehash(hash, &sig).unwrap();
            }
            o if o == EC_P384_OID => {
                let sig = p384::ecdsa::Signature::from_der(data(&resp)).unwrap();
                let vk = p384::ecdsa::VerifyingKey::from_sec1_bytes(der).unwrap();
                vk.verify_prehash(hash, &sig).unwrap();
            }
            _ => {
                let sig = p521::ecdsa::Signature::from_der(data(&resp)).unwrap();
                let vk = p521::ecdsa::VerifyingKey::from_sec1_bytes(der).unwrap();
                vk.verify_prehash(hash, &sig).unwrap();
            }
        }
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

    /// PSO signing a raw precomputed hash (ECDSA path).
    fn pso_sign_hash_apdu(hash: &[u8]) -> Vec<u8> {
        let mut apdu = vec![0x00, 0x2A, 0x9E, 0x9A, hash.len() as u8];
        apdu.extend_from_slice(hash);
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
        let (mut engine, _, _, _) = make_engine();
        let resp = engine.process_apdu(&apdu_select_gids(0x00));
        assert_eq!(status(&resp), 0x9000);
        assert_eq!(data(&resp), &[
            0x61, 0x12, 0x4F, 0x0B, 0xA0, 0x00, 0x00, 0x03, 0x97, 0x42, 0x54, 0x46, 0x59, 0x02,
            0x01, 0x73, 0x03, 0x40, 0x01, 0xC0,
        ]);
    }

    #[test]
    fn select_unknown_aid_returns_6a82() {
        let (mut engine, _, _, _) = make_engine();
        let resp = engine.process_apdu(&[0x00, 0xA4, 0x04, 0x00, 0x09, 0xA0, 0x00, 0x00, 0x03, 0x08, 0x00, 0x00, 0x10, 0x00]);
        assert_eq!(status(&resp), 0x6A82);
    }

    #[test]
    fn get_data_cardid_matches_reference() {
        let (mut engine, _, _, _) = make_engine();
        let resp = engine.process_apdu(&get_data_apdu(0xA0, 0x12, &[0xDF, 0x20]));
        assert_eq!(status(&resp), 0x9000);
        let d = data(&resp);
        assert_eq!(&d[..3], &[0xDF, 0x20, 0x10]);
        assert_eq!(&d[3..], &REFERENCE_CARDID);
    }

    #[test]
    fn get_data_cardapps_and_config() {
        let (mut engine, _, _, _) = make_engine();
        let resp = engine.process_apdu(&get_data_apdu(0xA0, 0x00, &[0xDF, 0x1F]));
        assert_eq!(status(&resp), 0x9000);
        assert_eq!(data(&resp)[..3], [0xDF, 0x1F, 0x81]);

        let resp = engine.process_apdu(&get_data_apdu(0xA0, 0x10, &[0xDF, 0x22]));
        assert_eq!(status(&resp), 0x9000);
        assert_eq!(data(&resp), &[0xDF, 0x22, 0x06, 0x00, 0x00, 0x01, 0x00, 0x05, 0x00]);
    }

    #[test]
    fn get_data_cmapfile_contains_reference_guid() {
        let (mut engine, _, _, _) = make_engine();
        let resp = engine.process_apdu(&get_data_apdu(0xA0, 0x10, &[0xDF, 0x23]));
        assert_eq!(status(&resp), 0x9000);
        let d = data(&resp);
        assert_eq!(&d[..3], &[0xDF, 0x23, 0x56]);
        let guid_utf16: Vec<u8> = REFERENCE_GUID.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(&d[3..3 + guid_utf16.len()], &guid_utf16);
    }

    #[test]
    fn get_data_7f73_is_not_found() {
        let (mut engine, _, _, _) = make_engine();
        let resp = engine.process_apdu(&get_data_apdu(0x3F, 0xFF, &[0x7F, 0x73]));
        assert_eq!(status(&resp), 0x6A88);
    }

    #[test]
    fn get_data_7f49_returns_pubkey_with_chaining() {
        let (mut engine, n, e, _) = make_engine();
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
        let (mut engine, _, _, _) = make_engine();
        let resp = engine.process_apdu(&get_data_apdu(0xA0, 0x10, &[0xDF, 0x24]));
        assert_eq!(status(&resp) >> 8, 0x61);
        assert_eq!(data(&resp)[..3], [0xDF, 0x24, 0x82]);
    }

    #[test]
    fn df24_content_is_01_00_len_zlib_compressed_cert() {
        use std::io::Read;
        let (mut engine, _, _, _) = make_engine();
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
    fn verify_with_unencrypted_key_always_ok() {
        // Key has no password -> the card needs no PIN, any VERIFY succeeds.
        let (mut engine, _, _, _) = make_engine();
        let resp = engine.process_apdu(&verify_apdu("whatever"));
        assert_eq!(status(&resp), 0x9000);
    }

    #[test]
    fn encrypted_key_requires_password_to_verify() {
        let mut engine = make_encrypted_engine("p4ss");
        // Wrong password -> 63 CX.
        let resp = engine.process_apdu(&verify_apdu("wrong"));
        assert_eq!(status(&resp) >> 8, 0x63);
        // PSO without a correct VERIFY -> 6982.
        let resp = engine.process_apdu(&pso_sign_apdu());
        assert_eq!(status(&resp), 0x6982);
        // Correct password -> 9000, then PSO signs.
        let resp = engine.process_apdu(&verify_apdu("p4ss"));
        assert_eq!(status(&resp), 0x9000);
        let resp = engine.process_apdu(&pso_sign_apdu());
        assert_eq!(status(&resp), 0x9000);
        assert_eq!(data(&resp).len(), 256);
    }

    #[test]
    fn empty_verify_is_status_query_and_does_not_decrement() {
        // msclmd sends `00 20 00 80` (Lc=0) as a PIN-status query before showing
        // the PIN dialog. It must report the remaining attempts WITHOUT consuming
        // one (a bug here blocked the card after a few probes).
        let mut engine = make_encrypted_engine("p4ss");
        let query = [0x00, 0x20, 0x00, 0x80];

        // Repeated queries keep reporting 3 attempts (no decrement).
        for _ in 0..5 {
            let resp = engine.process_apdu(&query);
            assert_eq!(status(&resp), 0x63C3);
        }

        // A real wrong PIN then consumes one attempt.
        let resp = engine.process_apdu(&verify_apdu("wrong"));
        assert_eq!(status(&resp), 0x63C2);

        // Queries keep reporting the current counter.
        let resp = engine.process_apdu(&query);
        assert_eq!(status(&resp), 0x63C2);

        // Exhaust the attempts -> blocked.
        engine.process_apdu(&verify_apdu("wrong"));
        engine.process_apdu(&verify_apdu("wrong"));
        let resp = engine.process_apdu(&verify_apdu("wrong"));
        assert_eq!(status(&resp), 0x6983);
        // And the query reports blocked too.
        let resp = engine.process_apdu(&query);
        assert_eq!(status(&resp), 0x6983);
    }

    #[test]
    fn mse_set_ok() {
        let (mut engine, _, _, _) = make_engine();
        let mse = [0x00, 0x22, 0x41, 0xB6, 0x06, 0x80, 0x01, 0x57, 0x84, 0x01, 0x81];
        assert_eq!(status(&engine.process_apdu(&mse)), 0x9000);
    }

    #[test]
    fn pso_sign_produces_valid_signature() {
        let (mut engine, n, e, _) = make_engine();
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
    fn from_spec_pem_parses() {
        let mut rng = rsa::rand_core::OsRng;
        let key = rsa::RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let key_pem = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let cert_pem = pem::Pem::new("CERTIFICATE", vec![0x30, 0x82, 0x01, 0x00]).to_string();
        let backend = crate::smartcard::emulated::EmulatedBackend::from_spec(&format!(
            "pem:{},{}",
            cert_pem, key_pem
        ));
        assert!(backend.is_some());
    }

    #[test]
    fn from_spec_unsupported_prefix_is_none() {
        let backend = crate::smartcard::emulated::EmulatedBackend::from_spec("userdefined:whatever");
        assert!(backend.is_none());
    }

    #[test]
    fn unknown_apdu_returns_6986() {
        let (mut engine, _, _, _) = make_engine();
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

    // ---------------------------------------------------------------------
    // ECDSA
    // ---------------------------------------------------------------------

    #[test]
    fn ecdsa_p256_sign_and_pubkey() {
        use p256::pkcs8::EncodePrivateKey;
        let mut rng = p256::elliptic_curve::rand_core::OsRng;
        let sk = p256::ecdsa::SigningKey::random(&mut rng);
        let point = p256::ecdsa::VerifyingKey::from(&sk).to_encoded_point(false).as_bytes().to_vec();
        let key_pem = sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let cert = build_min_ec_cert_der(&point, EC_P256_OID);
        let mut engine = GidsEngine::new(cert, key_pem).unwrap();
        assert_ec_engine(&mut engine, EC_P256_OID, &[0xAB; 32], &point);
    }

    #[test]
    fn ecdsa_p384_sign_and_pubkey() {
        use p384::pkcs8::EncodePrivateKey;
        let mut rng = p384::elliptic_curve::rand_core::OsRng;
        let sk = p384::ecdsa::SigningKey::random(&mut rng);
        let point = p384::ecdsa::VerifyingKey::from(&sk).to_encoded_point(false).as_bytes().to_vec();
        let key_pem = sk.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let cert = build_min_ec_cert_der(&point, EC_P384_OID);
        let mut engine = GidsEngine::new(cert, key_pem).unwrap();
        assert_ec_engine(&mut engine, EC_P384_OID, &[0xCD; 48], &point);
    }

    #[test]
    fn ecdsa_p521_sign_and_pubkey() {
        use p521::pkcs8::EncodePrivateKey;
        let mut rng = p521::elliptic_curve::rand_core::OsRng;
        let secret = p521::SecretKey::random(&mut rng);
        let sk = p521::ecdsa::SigningKey::from_bytes(&secret.to_bytes()).unwrap();
        let point = p521::ecdsa::VerifyingKey::from(&sk).to_encoded_point(false).as_bytes().to_vec();
        let key_pem = secret.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let cert = build_min_ec_cert_der(&point, EC_P521_OID);
        let mut engine = GidsEngine::new(cert, key_pem).unwrap();
        assert_ec_engine(&mut engine, EC_P521_OID, &[0xEF; 64], &point);
    }

    #[test]
    fn ecdsa_p256_encrypted_requires_pin() {
        use p256::pkcs8::EncodePrivateKey;
        use p256::ecdsa::signature::hazmat::PrehashVerifier;
        let mut rng = p256::elliptic_curve::rand_core::OsRng;
        let sk = p256::ecdsa::SigningKey::random(&mut rng);
        let point = p256::ecdsa::VerifyingKey::from(&sk).to_encoded_point(false).as_bytes().to_vec();
        let key_pem = sk
            .to_pkcs8_encrypted_pem(&mut rng, "p4ss", LineEnding::LF)
            .unwrap()
            .to_string();
        let cert = build_min_ec_cert_der(&point, EC_P256_OID);
        let mut engine = GidsEngine::new(cert, key_pem).unwrap();

        // Wrong PIN -> 63 CX; signing gated -> 6982.
        let resp = engine.process_apdu(&verify_apdu("wrong"));
        assert_eq!(status(&resp) >> 8, 0x63);
        let resp = engine.process_apdu(&pso_sign_hash_apdu(&[0xAB; 32]));
        assert_eq!(status(&resp), 0x6982);

        // Correct PIN -> 9000, then the signature verifies.
        let resp = engine.process_apdu(&verify_apdu("p4ss"));
        assert_eq!(status(&resp), 0x9000);
        let resp = engine.process_apdu(&pso_sign_hash_apdu(&[0xAB; 32]));
        assert_eq!(status(&resp), 0x9000);
        let sig = p256::ecdsa::Signature::from_der(data(&resp)).unwrap();
        let vk = p256::ecdsa::VerifyingKey::from_sec1_bytes(&point).unwrap();
        vk.verify_prehash(&[0xAB; 32], &sig).unwrap();
    }

    #[test]
    fn ecdsa_p521_encrypted_requires_pin() {
        use p521::pkcs8::EncodePrivateKey;
        use p521::ecdsa::signature::hazmat::PrehashVerifier;
        let mut rng = p521::elliptic_curve::rand_core::OsRng;
        let secret = p521::SecretKey::random(&mut rng);
        let sk = p521::ecdsa::SigningKey::from_bytes(&secret.to_bytes()).unwrap();
        let point = p521::ecdsa::VerifyingKey::from(&sk).to_encoded_point(false).as_bytes().to_vec();
        let key_pem = secret
            .to_pkcs8_encrypted_pem(&mut rng, "p4ss", LineEnding::LF)
            .unwrap()
            .to_string();
        let cert = build_min_ec_cert_der(&point, EC_P521_OID);
        let mut engine = GidsEngine::new(cert, key_pem).unwrap();

        // Wrong PIN -> 63 CX; signing gated -> 6982.
        let resp = engine.process_apdu(&verify_apdu("wrong"));
        assert_eq!(status(&resp) >> 8, 0x63);
        let resp = engine.process_apdu(&pso_sign_hash_apdu(&[0xEF; 64]));
        assert_eq!(status(&resp), 0x6982);

        // Correct PIN -> 9000, then the signature verifies (PBES2 manual decrypt).
        let resp = engine.process_apdu(&verify_apdu("p4ss"));
        assert_eq!(status(&resp), 0x9000);
        let resp = engine.process_apdu(&pso_sign_hash_apdu(&[0xEF; 64]));
        assert_eq!(status(&resp), 0x9000);
        let sig = p521::ecdsa::Signature::from_der(data(&resp)).unwrap();
        let vk = p521::ecdsa::VerifyingKey::from_sec1_bytes(&point).unwrap();
        vk.verify_prehash(&[0xEF; 64], &sig).unwrap();
    }
}
