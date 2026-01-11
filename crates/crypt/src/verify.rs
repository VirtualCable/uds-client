// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

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
