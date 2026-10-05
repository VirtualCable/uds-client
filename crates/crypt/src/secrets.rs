// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::Arc;

use anyhow::Result;

use aes_gcm::{Aes256Gcm, aead::KeyInit};
use hkdf::Hkdf;
use sha2::Sha256;

use shared::log;

use zeroize::Zeroize;

use crate::{
    datagram::DatagramCrypt,
    rekey::{
        DIR_LAUNCHER_TO_SERVER, DIR_SERVER_TO_LAUNCHER, RekeyState, SessionPrk, TRANSPORT_TCP,
        TRANSPORT_UDP,
    },
    tunnel::Crypt,
    types::{SharedSecret, Ticket},
};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct CryptoKeys {
    pub key_payload: SharedSecret,
    pub key_send: SharedSecret,
    pub key_receive: SharedSecret,
    pub nonce_payload: [u8; 12],
}

pub fn derive_tunnel_material(
    shared_secret: &SharedSecret,
    ticket_id: &Ticket,
) -> Result<CryptoKeys> {
    // Note: Ticket for data scripted fro script is DIFFERENT from the one used for tunnel
    // Do not mix them up :).
    log::debug!(
        "Deriving tunnel material with shared_secret: {:?} and ticket_id: {:?}",
        shared_secret,
        ticket_id
    );
    // HKDF-Extract + Expand with SHA-256
    let hk = Hkdf::<Sha256>::new(Some(ticket_id.as_ref()), shared_secret.as_ref());

    let mut okm = [0u8; 108];
    hk.expand(b"openuds-ticket-crypt", &mut okm)
        .map_err(|_| anyhow::format_err!("HKDF expand failed"))?;

    let mut key_payload = [0u8; 32];
    let mut key_send = [0u8; 32];
    let mut key_receive = [0u8; 32];
    let mut nonce_payload = [0u8; 12];

    key_payload.copy_from_slice(&okm[0..32]);
    key_send.copy_from_slice(&okm[32..64]);
    key_receive.copy_from_slice(&okm[64..96]);
    nonce_payload.copy_from_slice(&okm[96..108]);

    Ok(CryptoKeys {
        key_payload: key_payload.into(),
        key_send: key_send.into(),
        key_receive: key_receive.into(),
        nonce_payload,
    })
}

/// The rekeying states of the TCP leg, from the LAUNCHER point of view:
/// `inbound` decrypts server→launcher traffic (the `DIR_SERVER_TO_LAUNCHER`
/// epoch domain), `outbound` encrypts launcher→server traffic. Both share the
/// session PRK and the negotiated `k`; they differ only in direction (the
/// epoch-key domain separator).
///
/// `k` is announced by the server in the `OpenResponse` and adopted by the
/// proxy at the first handshake; on `Recover` the proxy installs the adopted
/// value *before* sending the handshake ticket, because on recovery the
/// session's counters are already past zero and the ticket frame itself may
/// belong to an epoch > 0.
pub struct TunnelRekeys {
    pub inbound: Arc<RekeyState>,
    pub outbound: Arc<RekeyState>,
}

impl TunnelRekeys {
    /// Build the launcher-perspective pair from the session PRK, the
    /// epoch-0 tunnel material and threshold `k` (0 = OFF: each state
    /// collapses to a single epoch whose cipher is the legacy tunnel key,
    /// byte-identical to a pre-rekeying wire).
    pub fn from_parts(keys: &CryptoKeys, prk: Arc<SessionPrk>, k: u8) -> Self {
        TunnelRekeys {
            inbound: Arc::new(RekeyState::new(
                prk.clone(),
                TRANSPORT_TCP,
                DIR_SERVER_TO_LAUNCHER,
                k,
                0,
                Arc::new(Aes256Gcm::new(keys.key_receive.as_ref().into())),
            )),
            outbound: Arc::new(RekeyState::new(
                prk,
                TRANSPORT_TCP,
                DIR_LAUNCHER_TO_SERVER,
                k,
                0,
                Arc::new(Aes256Gcm::new(keys.key_send.as_ref().into())),
            )),
        }
    }
}

/// Convenience wrapper deriving the PRK inline from the session material.
/// The production proxy path keeps the PRK alive on the `Proxy` and calls
/// [`TunnelRekeys::from_parts`]; this exists for tests and simple callers.
pub fn build_tunnel_rekeys(
    keys: &CryptoKeys,
    shared_secret: &SharedSecret,
    ticket_id: &Ticket,
    k: u8,
) -> TunnelRekeys {
    TunnelRekeys::from_parts(
        keys,
        Arc::new(SessionPrk::derive(shared_secret, ticket_id)),
        k,
    )
}

/// Returns (inbound, outbound) crypts
/// inbound: for reading from the tunnel (decrypting)
/// outbound: for writing to the tunnel (encrypting)
///
/// The crypts start on the legacy epoch-0 key. The launcher adopts the
/// server's `OpenResponse.rekey_log2` through [`Crypt::set_rekey`] after the
/// Open handshake (and installs it before the ticket of every `Recover`,
/// where the counters are already past zero).
/// # Arguments
/// * `keys` - Derived cryptographic keys
/// * `seqs` - Initial sequence numbers for (inbound, outbound) crypts
pub fn get_tunnel_crypts(keys: &CryptoKeys, seqs: (u64, u64)) -> Result<(Crypt, Crypt)> {
    log::debug!(
        "Derived tunnel material: key_receive={:?}, key_send={:?}",
        keys.key_receive,
        keys.key_send
    );

    let inbound = Crypt::new(&keys.key_receive, seqs.0);
    let outbound = Crypt::new(&keys.key_send, seqs.1);

    Ok((inbound, outbound))
}

/// Returns (inbound, outbound) UDP datagram crypts, from the LAUNCHER point of
/// view: inbound decrypts datagrams from the tunnel server (key_server_to_client),
/// outbound encrypts datagrams towards it (key_client_to_server). The server
/// uses the same two keys swapped (its send key is our inbound key).
///
/// Keys are derived with a dedicated HKDF label so they are independent from
/// the TCP leg keys even though both come from the same ticket shared secret
/// (domain separation). Sequence numbers also live in their own space: the
/// UDP leg does not interact with the stream `Crypt` seqs at all.
///
/// `k` is the same session-wide rekey threshold as the TCP leg (0 = OFF,
/// byte-identical to the legacy single-key construction). UDP seqs are
/// anchored at `INITIAL_SEQ`, so the epoch counting starts at the session's
/// first datagram whatever the absolute counter value. The replay window is
/// orthogonal to rekeying: it bounds *which* seqs are accepted; the epoch
/// only decides *which key* decrypts them.
pub fn get_udp_crypts(
    shared_secret: &SharedSecret,
    ticket_id: &Ticket,
    k: u8,
) -> Result<(DatagramCrypt, DatagramCrypt)> {
    let hk = Hkdf::<Sha256>::new(Some(ticket_id.as_ref()), shared_secret.as_ref());

    let mut okm = [0u8; 64];
    hk.expand(b"openuds-ticket-crypt-udp", &mut okm)
        .map_err(|_| anyhow::format_err!("HKDF expand failed"))?;

    let mut key_client_to_server = [0u8; 32];
    let mut key_server_to_client = [0u8; 32];
    key_client_to_server.copy_from_slice(&okm[0..32]);
    key_server_to_client.copy_from_slice(&okm[32..64]);
    okm.zeroize();

    let c2s = SharedSecret::new(key_client_to_server);
    let s2c = SharedSecret::new(key_server_to_client);

    if k == 0 {
        return Ok((DatagramCrypt::new(&s2c), DatagramCrypt::new(&c2s)));
    }

    let prk = Arc::new(SessionPrk::derive(shared_secret, ticket_id));
    let inbound = DatagramCrypt::with_rekey(Arc::new(RekeyState::new(
        prk.clone(),
        TRANSPORT_UDP,
        DIR_SERVER_TO_LAUNCHER,
        k,
        DatagramCrypt::INITIAL_SEQ_ANCHOR,
        Arc::new(Aes256Gcm::new(s2c.as_ref().into())),
    )));
    let outbound = DatagramCrypt::with_rekey(Arc::new(RekeyState::new(
        prk,
        TRANSPORT_UDP,
        DIR_LAUNCHER_TO_SERVER,
        k,
        DatagramCrypt::INITIAL_SEQ_ANCHOR,
        Arc::new(Aes256Gcm::new(c2s.as_ref().into())),
    )));

    Ok((inbound, outbound))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_tunnel_material() {
        let shared_secret = SharedSecret::new([1u8; 32]);
        let ticket: Ticket = [2u8; 48].into();

        let material = derive_tunnel_material(&shared_secret, &ticket).unwrap();

        // Verify derived keys, known values
        assert_eq!(
            *material.key_send.as_ref(),
            [
                165, 213, 31, 20, 62, 238, 14, 209, 50, 193, 226, 239, 216, 45, 76, 37, 101, 11,
                173, 113, 185, 254, 51, 7, 50, 39, 232, 253, 55, 12, 21, 156
            ]
        );
        assert_eq!(
            *material.key_receive.as_ref(),
            [
                30, 79, 83, 235, 53, 71, 186, 71, 34, 250, 3, 51, 222, 193, 90, 208, 48, 112, 207,
                208, 219, 166, 191, 4, 208, 106, 159, 121, 221, 115, 30, 174
            ]
        );
    }

    #[test]
    fn test_get_tunnel_crypts() {
        let shared_secret = SharedSecret::new([1u8; 32]);
        let ticket: Ticket = [2u8; 48].into();

        let crypto_keys = derive_tunnel_material(&shared_secret, &ticket).unwrap();

        let (inbound, outbound) = get_tunnel_crypts(&crypto_keys, (0, 0)).unwrap();

        assert_eq!(inbound.current_seq(), 0);
        assert_eq!(outbound.current_seq(), 0);
    }

    /// Known-answer test for the UDP leg key derivation. The exact same
    /// expected values live in the tunnel-server's shared crate, so a drift in
    /// either implementation breaks a build. Expected values produced by an
    /// independent HKDF-SHA256 implementation (RFC 5869).
    #[test]
    fn test_get_udp_crypts_known_answer() {
        use crate::datagram::{DatagramCrypt, TOKEN_LENGTH};

        let shared_secret = SharedSecret::new([1u8; 32]);
        let ticket: Ticket = [2u8; 48].into();

        let (mut inbound, mut outbound) = get_udp_crypts(&shared_secret, &ticket, 0).unwrap();

        let token = [0x42u8; TOKEN_LENGTH];

        // Outbound (client -> server) must use the expected c2s key: a crypt
        // built directly on the known c2s key must produce the same datagram.
        let expected_c2s: [u8; 32] = [
            165, 215, 81, 8, 62, 101, 176, 192, 153, 20, 87, 9, 192, 41, 1, 145, 120, 68, 37, 43,
            6, 56, 160, 235, 231, 173, 137, 157, 132, 240, 48, 25,
        ];
        let mut reference = DatagramCrypt::new(&SharedSecret::new(expected_c2s));
        assert_eq!(
            reference.encrypt(&token, b"kat").unwrap(),
            outbound.encrypt(&token, b"kat").unwrap()
        );

        // Inbound (server -> client) must use the expected s2c key: it must
        // decrypt a datagram produced with the known s2c key.
        let expected_s2c: [u8; 32] = [
            115, 122, 103, 8, 221, 26, 166, 141, 102, 141, 74, 208, 99, 240, 91, 76, 233, 111, 200,
            0, 152, 79, 177, 241, 178, 56, 195, 87, 176, 182, 35, 9,
        ];
        let mut reference = DatagramCrypt::new(&SharedSecret::new(expected_s2c));
        let datagram = reference.encrypt(&token, b"kat").unwrap();
        assert_eq!(
            inbound.decrypt(&token, &datagram).unwrap().as_deref(),
            Some(b"kat".as_slice())
        );
    }

    /// The UDP keys must differ from the TCP leg keys (domain separation).
    #[test]
    fn test_udp_keys_differ_from_tcp_keys() {
        use crate::datagram::{DatagramCrypt, TOKEN_LENGTH};

        let shared_secret = SharedSecret::new([1u8; 32]);
        let ticket: Ticket = [2u8; 48].into();

        let material = derive_tunnel_material(&shared_secret, &ticket).unwrap();
        let (_udp_in, mut udp_out) = get_udp_crypts(&shared_secret, &ticket, 0).unwrap();
        let token = [7u8; TOKEN_LENGTH];

        // A datagram encrypted with the UDP outbound key must not verify
        // under a crypt built with any of the TCP leg keys.
        let datagram = udp_out.encrypt(&token, b"x").unwrap();
        for key in [
            &material.key_send,
            &material.key_receive,
            &material.key_payload,
        ] {
            let mut wrong = DatagramCrypt::new(key);
            assert!(wrong.decrypt(&token, &datagram).is_err());
        }
    }

    // This will not compile, as ticket length is enforced by type
    // #[test]
    // fn test_invalid_ticket_length() {
    //     let shared_secret = [1u8; 32];
    //     let ticket_id = [2u8; 16]; // Too short

    //     let result = get_tunnel_crypts(&shared_secret, &ticket_id);
    //     assert!(result.is_err());
    // }
}
