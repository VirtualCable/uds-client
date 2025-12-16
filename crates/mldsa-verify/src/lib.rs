// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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

use ml_dsa::{
    EncodedSignature, EncodedVerifyingKey, MlDsa65, Signature, VerifyingKey, signature::Verifier,
};

// UDS Client ML DSA Public Key
static PUBLIC_KEY: &[u8] = include_bytes!("../public-key.bin");

pub fn verify_signature(message: &[u8], signature_b64: &str) -> Result<()> {
    use base64::engine::{Engine as _, general_purpose::STANDARD};

    let encoded_vk = EncodedVerifyingKey::<MlDsa65>::try_from(PUBLIC_KEY).map_err(|e| {
        anyhow::anyhow!(
            "Failed to decode ML DSA verifying key from bytes: {}",
            e
        )
    })?;
    let recovered_vk = VerifyingKey::<MlDsa65>::decode(&encoded_vk);
    let signature_bytes = STANDARD.decode(signature_b64).map_err(|e| {
        anyhow::anyhow!("Failed to decode signature from base64: {}", e)
    })?;
    let signature_enc: EncodedSignature<MlDsa65> =
        EncodedSignature::<MlDsa65>::try_from(signature_bytes.as_slice()).map_err(|e| {
            anyhow::anyhow!("Failed to decode ML DSA signature from bytes: {}", e)
        })?;
    let signature = Signature::<MlDsa65>::decode(&signature_enc).ok_or_else(|| {
        anyhow::anyhow!("Failed to recover ML DSA signature from encoded signature")
    })?;
    recovered_vk
        .verify(message, &signature)
        .map_err(|e| anyhow::anyhow!("Signature verification failed: {}", e))
}
