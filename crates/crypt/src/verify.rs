// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::Result;
use base64::engine::{Engine as _, general_purpose::STANDARD};

use libcrux_ml_dsa::ml_dsa_65;

// UDS Client ML-DSA Public Key
static PUBLIC_KEY: &[u8] = include_bytes!("../public-key.bin");

pub fn verify_signature(message: &[u8], signature_b64: &str) -> Result<()> {
    // If public key len is not correct, return error
    let public_key: [u8; 1952] = PUBLIC_KEY
        .try_into()
        .map_err(|_| anyhow::anyhow!("Failed to convert public key bytes into array"))?;
    let pk = ml_dsa_65::MLDSA65VerificationKey::new(public_key);

    // 2. Decodificar firma desde base64
    let sig_bytes = STANDARD
        .decode(signature_b64)
        .map_err(|e| anyhow::anyhow!("Failed to decode signature from base64: {}", e))?;
    // Must have exactly 3309 bytes

    if sig_bytes.len() != 3309 {
        return Err(anyhow::anyhow!(
            "Invalid signature length: expected 3309 bytes, got {} bytes",
            sig_bytes.len()
        ));
    }
    let sig_bytes: [u8; 3309] = sig_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Failed to convert signature bytes into array"))?;

    // 3. Parsear firma desde bytes
    let signature = ml_dsa_65::MLDSA65Signature::new(sig_bytes);
    // 4. Verificar firma
    ml_dsa_65::verify(&pk, message, &[], &signature)
        .map_err(|_| anyhow::anyhow!("Signature verification failed"))
}
