use anyhow::Result;

use sha2::digest::typenum;

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine as _, engine::general_purpose};

use shared::log;
use crypt::consts::{CIPHERTEXT_SIZE, PRIVATE_KEY_SIZE};
use crypt::{
    kem::{CipherText, PrivateKey, decapsulate},
    secrets::derive_tunnel_material,
};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct BrokerTicket {
    pub algorithm: String,
    pub ciphertext: String,
    pub data: String,
}

impl BrokerTicket {
    pub fn new(algorithm: &str, ciphertext: &str, data: &str) -> Self {
        BrokerTicket {
            algorithm: algorithm.to_string(),
            ciphertext: ciphertext.to_string(),
            data: data.to_string(),
        }
    }

    pub fn recover_data_from_json(
        &self,
        ticket_id: &str,
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
        // As long as the sizes are correct (that will be for sure)
        let shared_secret = decapsulate(&kem_private_key, &kem_ciphertext).into();

        log::debug!(
            "Recovered shared secret: {:?}, ticket_id: {:?}",
            shared_secret,
            ticket_id
        );

        let data = general_purpose::STANDARD
            .decode(&self.data)
            .map_err(|e| anyhow::format_err!("Failed to decode base64 data: {}", e))?;

        // Derive tunnel material for decryption of data
        let material = derive_tunnel_material(&shared_secret, &ticket_id.as_bytes().try_into()?)?;

        let cipher = Aes256Gcm::new(material.key_payload.as_ref().into());
        let nonce: &Nonce<typenum::U12> = Nonce::from_slice(material.nonce_payload.as_ref());
        let plaintext = cipher
            .decrypt(nonce, data.as_ref())
            .map_err(|_| anyhow::format_err!("AES-256-GCM decryption failed"))?;
        let mut json_value: serde_json::Value = serde_json::from_slice(&plaintext)
            .map_err(|_| anyhow::format_err!("Failed to parse JSON from decrypted data"))?;

        // Create a shared_secret field, insert the values and add to json_value
        json_value["shared_secret"] = serde_json::to_value(shared_secret)?;

        log::debug!(
            "Recovered data from ticket: {:?}, ticket_id: {:?}",
            json_value,
            ticket_id
        );

        Ok(json_value)
    }
}
