use anyhow::Result;

use rand::{prelude::*, rngs::StdRng};

// reexport Ciphertext, PrivateKey, decapsulate, generate_keypair
pub use libcrux_ml_kem::mlkem768::{
    MlKem768Ciphertext as CipherText, MlKem768PrivateKey as PrivateKey, decapsulate,
};

use libcrux_ml_kem::mlkem768::generate_key_pair as ml_kem_generate_key_pair;

// Note, changes to kem size (1024, 768 or 512) will need to update also SECRET_KEY_SIZE and CIPHERTEXT_SIZE

/// Generate a new KEM keypair (private key and public key)
pub fn generate_key_pair() -> Result<(Vec<u8>, Vec<u8>)> {
    let mut rng: StdRng = rand::make_rng();
    let mut randomness = [0u8; 64];
    rng.fill_bytes(&mut randomness);
    let keypair = ml_kem_generate_key_pair(randomness);
    Ok((
        keypair.private_key().as_slice().to_vec(),
        keypair.public_key().as_slice().to_vec(),
    ))
}
