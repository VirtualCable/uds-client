// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
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

use aes_gcm::{AeadInPlace, Aes256Gcm, Nonce, aead::KeyInit};

use shared::log;

use crate::types::SharedSecret;

// Comms related
pub mod consts;
pub mod stream;
pub mod types;

pub struct Crypt {
    key: SharedSecret,
    cipher: Aes256Gcm,
    seq: u64,
}

impl Crypt {
    pub fn new(key: &SharedSecret, seq: u64) -> Self {
        log::debug!("Creating Crypt with initial seq: {}", seq);
        let cipher = Aes256Gcm::new(key.as_ref().into());
        Crypt {
            key: *key,
            cipher,
            seq,
        }
    }

    /// Increments and returns the internal seq.
    /// Note: the encrypt method automatically calls this method to get a unique seq for each encryption.
    /// Returns the incremented seq value.
    pub fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }

    /// Returns the current seq value without incrementing it.
    pub fn current_seq(&self) -> u64 {
        self.seq
    }

    /// Encrypts the given plaintext using AES-GCM with a unique nonce derived from an internal seq.
    /// The nonce is constructed by taking the current seq value and padding it to 12 bytes
    /// with zeros. The seq value is also used as associated data (AAD) to ensure integrity.
    /// Returns the ciphertext on success.
    /// The encryption is done inplace to avoid extra allocations.
    ///
    /// Note: length is the length of the plaintext data to encrypt.
    ///       also, the real data is written into buffer[2..], so first 2 bytes are free for channel id
    pub fn encrypt(
        &mut self,
        channel_id: u16,
        len: usize,
        buffer: &mut types::PacketBuffer,
    ) -> Result<usize> {
        types::PacketBuffer::ensure_capacity(len + consts::TAG_LENGTH)?;

        // Set the channel id in the buffer (first 2 bytes of the data part, after header)
        buffer.set_channel_id(channel_id);

        let data_with_channel_length = types::PacketBuffer::calc_data_with_channel_len(len)?;

        let seq = self.next_seq();
        buffer.set_seq(seq);
        buffer.set_length(data_with_channel_length + consts::TAG_LENGTH)?; // Write header with seq and length of encrypted data

        let mut nonce = [0; 12];
        nonce[..8].copy_from_slice(&seq.to_be_bytes());
        let aad = &seq.to_be_bytes();

        // Get pointer to data part of the buffer, where encryption will happen
        let data = buffer.data_with_channel_mut();
        // Calculate the length of the data + channel part, which is what will be encrypted and tagged

        let tag = self
            .cipher
            .encrypt_in_place_detached(
                Nonce::from_slice(&nonce),
                aad,
                &mut data[..data_with_channel_length],
            )
            .map_err(|e| anyhow::anyhow!("encryption failure: {:?}", e))?;
        data[data_with_channel_length..data_with_channel_length + consts::TAG_LENGTH]
            .copy_from_slice(&tag);
        log::debug!(
            "ENC: seq {}, length {}, channel {}",
            seq,
            len,
            channel_id,
        );
        // Returns the FULL length of the encrypted packet (header + data + channel + tag)
        Ok(data_with_channel_length + consts::TAG_LENGTH)
    }

    /// Decrypts the given ciphertext using AES-GCM with a nonce derived from the provided seq.
    /// The nonce is constructed by taking the seq value and padding it to 12 bytes with
    /// zeros. The seq value is also used as associated data (AAD) to ensure integrity.
    /// Returns the decrypted plaintext on success, and the channel (first 2 bytes, little-endian u16).
    /// Note: length is the length on encrpypted data WITH the tag (so, as readed from the stream).
    pub fn decrypt(&mut self, buffer: &mut types::PacketBuffer) -> Result<()> {
        let seq = buffer.seq()?;
        if seq < self.current_seq() {
            return Err(anyhow::anyhow!(
                "replay attack detected: seq {} is less than current seq {}",
                seq,
                self.current_seq()
            ));
        }

        let length = buffer.length()?;
        if length < (consts::TAG_LENGTH + 2) {
            return Err(anyhow::anyhow!(
                "decryption failure: ciphertext too short: {} bytes",
                length
            ));
        }

        let len = length - consts::TAG_LENGTH;
        let chan_data_buffer = buffer.data_with_channel_mut();

        let mut nonce = [0; 12];
        nonce[..8].copy_from_slice(&seq.to_be_bytes());
        let aad = &seq.to_be_bytes();

        // Split ciphertext and tag
        let (ciphertext, rest) = chan_data_buffer.split_at_mut(len);
        let tag = &rest[..16];

        self.cipher
            .decrypt_in_place_detached(Nonce::from_slice(&nonce), aad, ciphertext, tag.into())
            .map_err(|e| anyhow::anyhow!("decryption failure: {:?}", e))?;

        self.seq = seq + 1; // Update to last used seq + 1, so no replays are possible

        // Fix data length to remove ending tag, so only channel + data is left
        buffer.set_length(len)?;

        log::debug!(
            "DEC: seq {}, length {}, channel {}",
            seq,
            len,
            buffer.channel_id(),
        );
        Ok(())
    }
}

impl Clone for Crypt {
    fn clone(&self) -> Self {
        log::debug!("Cloning Crypt with seq: {}", self.seq);
        let cipher = Aes256Gcm::new(self.key.as_ref().into());
        Crypt {
            cipher,
            key: self.key,
            seq: self.seq,
        }
    }
}

pub fn parse_header(buffer: &[u8]) -> Result<(u64, u16)> {
    if buffer.len() < 10 {
        return Err(anyhow::anyhow!("buffer too small for header"));
    }
    let seq = u64::from_be_bytes(buffer[0..8].try_into().unwrap());
    let length = u16::from_be_bytes(buffer[8..10].try_into().unwrap());
    if length as usize > consts::MAX_PACKET_SIZE {
        return Err(anyhow::anyhow!("invalid packet length: {}", length));
    }
    Ok((seq, length))
}

pub fn build_header(seq: u64, length: u16, buffer: &mut [u8]) -> Result<()> {
    if buffer.len() < 10 {
        return Err(anyhow::anyhow!("buffer too small for header"));
    }
    buffer[0..8].copy_from_slice(&seq.to_be_bytes());
    buffer[8..10].copy_from_slice(&length.to_be_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::types::SharedSecret;

    use super::*;
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn test_send_sync() {
        assert_send::<Crypt>();
        assert_sync::<Crypt>();
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        log::setup_logging("debug", log::LogType::Test);

        let key = SharedSecret::new([7u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        let mut buf = types::PacketBuffer::new();
        let plaintext = b"16 length text!!";
        buf.set_data(plaintext).unwrap();

        // Packet buffer will contain the header + the crypted data + tag
        crypt.encrypt(1, plaintext.len(), &mut buf).unwrap();

        let mut buf2 = buf.clone(); // copy the buffer
        crypt.decrypt(&mut buf2).unwrap();

        assert_eq!(buf2.data(), plaintext);
        assert_eq!(buf2.channel_id(), 1);
    }

    #[test]
    fn test_sequence_increments() {
        let key = SharedSecret::new([1u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        assert_eq!(crypt.current_seq(), 0);
        assert_eq!(crypt.next_seq(), 1);
        assert_eq!(crypt.next_seq(), 2);
        assert_eq!(crypt.current_seq(), 2);
    }

    #[test]
    fn test_replay_fails() {
        let key = SharedSecret::new([2u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        let mut buf = types::PacketBuffer::new();
        buf.set_data(b"abc").unwrap();

        crypt.encrypt(2, 3, &mut buf).unwrap();
        assert_eq!(buf.seq().unwrap(), crypt.current_seq());
        assert_eq!(
            buf.length().unwrap(),
            types::PacketBuffer::calc_data_with_channel_len(3).unwrap() + consts::TAG_LENGTH
        );

        let mut buf2 = buf.clone(); // clone the buffer

        // First decrypt should work
        crypt.decrypt(&mut buf2).unwrap();

        // Second decrypt with the same seq should fail
        let mut buf3 = buf.clone(); // clone the original buffer again
        let result = crypt.decrypt(&mut buf3).unwrap_err();

        assert!(
            result.to_string().contains("replay attack detected"),
            "{}",
            result
        );
    }

    #[test]
    fn test_decrypt_fails_on_bad_tag() {
        log::setup_logging("debug", log::LogType::Test);

        let key = SharedSecret::new([3u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        let mut buf = types::PacketBuffer::new();
        buf.set_data(b"hola").unwrap();

        let length = crypt.encrypt(3, 4, &mut buf).unwrap();
        assert_eq!(buf.seq().unwrap(), crypt.current_seq());
        assert_eq!(buf.length().unwrap(), length); // Length of encrypted data + tag

        let mut corrupted = buf.clone();
        // flip some bits at the end of the tag ("data" length = channel (2 bytes) + data (4 bytes) = 6 + tag (16 bytes) = 22 bytes)
        let data_len = length - 2; // data points to data, not channel id, but length includes channel id length
        corrupted.data_mut()[data_len - 1] ^= 0xFF; // flip bit in the tag

        let err = crypt.decrypt(&mut corrupted).unwrap_err();

        assert!(err.to_string().contains("decryption failure"), "{}", err);
    }

    #[test]
    fn test_decrypt_fails_on_truncated_ciphertext() {
        let key = SharedSecret::new([4u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        let mut buf = types::PacketBuffer::new();
        buf.set_data(b"hola").unwrap();

        crypt.encrypt(3, 4, &mut buf).unwrap();

        let mut truncated = buf.clone();
        truncated.set_length(buf.length().unwrap() - 5).unwrap(); // Set length to 2, which is less than the required 2 (channel) + 16 (tag)

        let err = crypt.decrypt(&mut truncated).unwrap_err();

        assert!(
            err.to_string().contains("ciphertext too short"),
            "{:?}",
            err
        );
    }

    #[test]
    fn test_encrypt_does_not_overwrite_extra_bytes() {
        let key = SharedSecret::new([9u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        let mut buf = types::PacketBuffer::new();
        buf.full_buffer_mut().fill(0xAF); // Fill with known pattern

        let before = buf.data_with_channel().to_vec();

        // Channel 32, 4 bytes of data
        let _ = crypt.encrypt(32, 5, &mut buf).unwrap();

        let after = buf.data_with_channel();

        // Just first 5 +  2 + 16 bytes can be changed (channel + data + tag)
        assert_eq!(&before[23..], &after[23..]);
    }
    #[test]
    fn test_encrypt_produces_unique_nonces() {
        let key = SharedSecret::new([10u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        let mut buf1 = types::PacketBuffer::new();
        buf1.set_data(b"a").unwrap();
        crypt.encrypt(1, 1, &mut buf1).unwrap();
        let c1 = buf1.buffer().unwrap().to_vec();

        let mut buf2 = types::PacketBuffer::new();
        buf2.set_data(b"a").unwrap();
        crypt.encrypt(1, 1, &mut buf2).unwrap();
        let c2 = buf2.buffer().unwrap().to_vec();
        assert_ne!(c1, c2);
    }
}
