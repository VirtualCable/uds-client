use anyhow::Result;

use hkdf::Hkdf;
use sha2::{Sha256, digest::typenum};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine as _, engine::general_purpose};

use super::kem::{CIPHERTEXT_SIZE, CipherText, PrivateKey, PRIVATE_KEY_SIZE, decapsulate};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct TunnelMaterial {
    pub key_payload: [u8; 32],
    pub key_send: [u8; 32],
    pub key_receive: [u8; 32],
    pub nonce_payload: [u8; 12],
}

pub(crate) fn derive_tunnel_material(shared_secret: &[u8], ticket_id: &[u8]) -> Result<TunnelMaterial> {
    if ticket_id.len() < 48 {
        anyhow::bail!("ticket_id must be at least 48 bytes");
    }

    // HKDF-Extract + Expand with SHA-256
    let hk = Hkdf::<Sha256>::new(Some(ticket_id), shared_secret);

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

    Ok(TunnelMaterial {
        key_payload,
        key_send,
        key_receive,
        nonce_payload,
    })
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Ticket {
    pub algorithm: String,
    pub ciphertext: String,
    pub data: String,
}

impl Ticket {
    pub fn new(algorithm: &str, ciphertext: &str, data: &str) -> Self {
        Ticket {
            algorithm: algorithm.to_string(),
            ciphertext: ciphertext.to_string(),
            data: data.to_string(),
        }
    }

    pub fn recover_data_from_json(
        &self,
        ticket_id: &[u8],
        kem_private_key: &[u8; PRIVATE_KEY_SIZE],
    ) -> Result<serde_json::Value> {
        let kem_private_key = PrivateKey::from(kem_private_key);

        // Extract shared_secret from KEM ciphertext
        let kem_ciphertext_bytes: [u8; CIPHERTEXT_SIZE] = general_purpose::STANDARD
            .decode(&self.ciphertext)
            .map_err(|e| anyhow::format_err!("Failed to decode base64 ciphertext: {}", e))?
            .try_into()
            .map_err(|_| anyhow::format_err!("Invalid ciphertext size"))?;

        let kem_ciphertext = CipherText::from(&kem_ciphertext_bytes);
        // Note, the opoeration will always succeed, even for invalid ciphertexts
        // As long as the sizes are correct (that will bee for sure)
        let shared_secret = decapsulate(&kem_private_key, &kem_ciphertext);

        let data = general_purpose::STANDARD
            .decode(&self.data)
            .map_err(|e| anyhow::format_err!("Failed to decode base64 data: {}", e))?;

        // Derive tunnel material
        let material = derive_tunnel_material(&shared_secret, ticket_id)?;

        let cipher = Aes256Gcm::new(material.key_payload.as_ref().into());
        let nonce: &Nonce<typenum::U12> = Nonce::from_slice(material.nonce_payload.as_ref());
        let plaintext = cipher
            .decrypt(nonce, data.as_ref())
            .map_err(|_| anyhow::format_err!("AES-256-GCM decryption failed"))?;
        let mut json_value: serde_json::Value = serde_json::from_slice(&plaintext)
            .map_err(|_| anyhow::format_err!("Failed to parse JSON from decrypted data"))?;

        // Create a crypto_params field, insert the values and add to json_value
        json_value["crypto_params"] = serde_json::to_value(&material)?;

        Ok(json_value)
    }
}
