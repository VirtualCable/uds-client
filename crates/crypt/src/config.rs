use anyhow::Result;

use hkdf::Hkdf;
use sha2::Sha256;

use crate::types::SharedSecret;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct CryptoConfig {
    pub key_payload: SharedSecret,
    pub key_send: SharedSecret,
    pub key_receive: SharedSecret,
    pub nonce_payload: [u8; 12],
}

pub fn derive_tunnel_material(
    shared_secret: &SharedSecret,
    ticket_id: &[u8],
) -> Result<CryptoConfig> {
    if ticket_id.len() < 48 {
        anyhow::bail!("ticket_id must be at least 48 bytes");
    }

    // HKDF-Extract + Expand with SHA-256
    let hk = Hkdf::<Sha256>::new(Some(ticket_id), shared_secret.as_ref());

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

    Ok(CryptoConfig {
        key_payload: key_payload.into(),
        key_send: key_send.into(),
        key_receive: key_receive.into(),
        nonce_payload,
    })
}
