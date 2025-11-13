use anyhow::Result;

use ml_dsa::{
    EncodedSignature, EncodedVerifyingKey, MlDsa65, Signature, VerifyingKey, signature::Verifier,
};

// UDS Client ML DSA Public Key
static PUBLIC_KEY: &[u8] = include_bytes!("../public-key.bin");

pub fn verify_signature(message: &[u8], signature_b64: &str) -> Result<()> {
    use base64::engine::{Engine as _, general_purpose::STANDARD};

    let encoded_vk = EncodedVerifyingKey::<MlDsa65>::try_from(PUBLIC_KEY).unwrap();
    let recovered_vk = VerifyingKey::<MlDsa65>::decode(&encoded_vk);
    let signature_bytes = STANDARD.decode(signature_b64).unwrap();
    let signature_enc: EncodedSignature<MlDsa65> =
        EncodedSignature::<MlDsa65>::try_from(signature_bytes.as_slice()).unwrap();
    let signature = Signature::<MlDsa65>::decode(&signature_enc).unwrap();
    recovered_vk
        .verify(message, &signature)
        .map_err(|e| anyhow::anyhow!("Signature verification failed: {}", e))
}
