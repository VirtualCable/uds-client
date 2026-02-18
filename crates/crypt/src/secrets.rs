use anyhow::Result;

use hkdf::Hkdf;
use sha2::Sha256;

use shared::log;

use crate::{types::{SharedSecret, Ticket}, tunnel::Crypt};

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Copy)]
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

/// Returns (inbound, outbound) crypts
/// inbound: for reading from the tunnel (decrypting)
/// outbound: for writing to the tunnel (encrypting)
/// # Arguments
/// * `keys` - Derived cryptographic keys
/// * `seqs` - Initial sequence numbers for (inbound, outbound) crypts
pub fn get_tunnel_crypts(
    keys: &CryptoKeys,
    seqs: (u64, u64),
) -> Result<(Crypt, Crypt)> {
    log::debug!(
        "Derived tunnel material: key_receive={:?}, key_send={:?}",
        keys.key_receive,
        keys.key_send
    );

    let inbound = Crypt::new(&keys.key_receive, seqs.0);
    let outbound = Crypt::new(&keys.key_send, seqs.1);

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

    // This will not compile, as ticket length is enforced by type
    // #[test]
    // fn test_invalid_ticket_length() {
    //     let shared_secret = [1u8; 32];
    //     let ticket_id = [2u8; 16]; // Too short

    //     let result = get_tunnel_crypts(&shared_secret, &ticket_id);
    //     assert!(result.is_err());
    // }
}
