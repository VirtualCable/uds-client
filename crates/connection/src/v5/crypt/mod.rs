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

use super::protocol::Command;

// Comms related
pub mod consts;
pub mod stream;
pub mod tunnel;
pub mod types;

pub struct Crypt {
    key: types::SharedSecret,
    cipher: Aes256Gcm,
    seq: u64,
}


impl Crypt {
    pub fn new(key: &types::SharedSecret, seq: u64) -> Self {
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
    pub fn encrypt<'a>(
        &mut self,
        channel: u16,
        len: usize,
        buffer: &'a mut types::PacketBuffer,
    ) -> Result<&'a [u8]> {
        let len = len + 2; // +2 for channel bytes, buffer alreasy has space for it in the beginning
        buffer.ensure_capacity(len + consts::TAG_LENGTH)?;

        // Get the slice to encrypt
        let buffer = buffer.stream_slice();
        buffer[0..2].copy_from_slice(&channel.to_be_bytes());

        let seq = self.next_seq();
        let mut nonce = [0; 12];
        nonce[..8].copy_from_slice(&seq.to_be_bytes());
        let aad = &seq.to_be_bytes();

        let tag = self
            .cipher
            .encrypt_in_place_detached(Nonce::from_slice(&nonce), aad, &mut buffer[..len])
            .map_err(|e| anyhow::anyhow!("encryption failure: {:?}", e))?;
        buffer[len..len + 16].copy_from_slice(&tag);
        Ok(&buffer[..len + 16])
    }

    /// Decrypts the given ciphertext using AES-GCM with a nonce derived from the provided seq.
    /// The nonce is constructed by taking the seq value and padding it to 12 bytes with
    /// zeros. The seq value is also used as associated data (AAD) to ensure integrity.
    /// Returns the decrypted plaintext on success, and the channel (first 2 bytes, little-endian u16).
    /// Note: length is the length on encrpypted data WITH the tag (so, as readed from the stream).
    pub fn decrypt<'a>(
        &mut self,
        seq: u64,
        length: u16,
        buffer: &'a mut types::PacketBuffer,
    ) -> Result<(&'a [u8], u16)> {
        if seq < self.seq {
            // Note: Due to recovery feature, we may ignore
            //       this error and return an empty payload, but log it as warning,
            log::warn!(
                "Out of order packet received: seq {} < current {}",
                seq,
                self.seq
            );
            // Mark as NOP, so it will be ignored by upper layers
            let cmd = Command::Nop.to_bytes();
            let buffer = &mut buffer.stream_slice()[..cmd.len()];
            buffer.copy_from_slice(cmd.as_slice());
            return Ok((buffer, 0)); // Empty vector indicates
        }
        self.seq = seq + 1; // Update to last used seq + 1, so no replays are possible
        if length < (consts::TAG_LENGTH + 2) as u16 {
            return Err(anyhow::anyhow!(
                "decryption failure: ciphertext too short: {} bytes",
                length
            ));
        }

        let len = (length as usize) - consts::TAG_LENGTH;
        let buffer = buffer.stream_slice();

        let mut nonce = [0; 12];
        nonce[..8].copy_from_slice(&seq.to_be_bytes());
        let aad = &seq.to_be_bytes();

        // Split ciphertext and tag
        let (ciphertext, rest) = buffer.split_at_mut(len);
        let tag = &rest[..16];

        self.cipher
            .decrypt_in_place_detached(Nonce::from_slice(&nonce), aad, ciphertext, tag.into())
            .map_err(|e| anyhow::anyhow!("decryption failure: {:?}", e))?;
        // First two bytes are channel
        let channel = u16::from_be_bytes(ciphertext[..2].try_into().map_err(|e| {
            anyhow::anyhow!(
                "decryption failure: failed to extract channel from decrypted data: {:?}",
                e
            )
        })?);
        Ok((&buffer[2..len], channel))
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
    use super::types::SharedSecret;

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
        buf.store(plaintext).unwrap();

        let ciphertext = crypt.encrypt(1, plaintext.len(), &mut buf).unwrap();

        // Now decrypt
        let seq = crypt.current_seq();
        let length = ciphertext.len() as u16; // Note: ciphertext includes channel + tag

        let mut buf2 = types::PacketBuffer::from(ciphertext);
        let (decrypted, channel) = crypt.decrypt(seq, length, &mut buf2).unwrap();

        assert_eq!(decrypted, plaintext);
        assert_eq!(channel, 1);
    }

    #[test]
    fn test_parse_build_header() {
        let mut buf = [0u8; 10];
        build_header(0x9922334455667788, 0x00AA, &mut buf).unwrap();

        let (seq, len) = parse_header(&buf).unwrap();
        assert_eq!(seq, 0x9922334455667788);
        assert_eq!(len, 0x00AA);
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
    fn test_replay_returns_nop() {
        let key = SharedSecret::new([2u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        let mut buf = types::PacketBuffer::new();
        buf.store(b"abc").unwrap();

        let ciphertext = crypt.encrypt(2, 3, &mut buf).unwrap();
        let seq = crypt.current_seq();

        let mut buf2 = types::PacketBuffer::from(ciphertext);

        // First decrypt should work
        crypt
            .decrypt(seq, ciphertext.len() as u16, &mut buf2)
            .unwrap();

        // Second decrypt with the same seq should fail
        let mut buf3 = types::PacketBuffer::from(ciphertext);
        let result = crypt
            .decrypt(seq, ciphertext.len() as u16, &mut buf3)
            .unwrap();

        assert_eq!(result.0, Command::Nop.to_bytes()); // Should return NOP command
    }

    #[test]
    fn test_decrypt_fails_on_bad_tag() {
        log::setup_logging("debug", log::LogType::Test);

        let key = SharedSecret::new([3u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        let mut buf = types::PacketBuffer::new();
        buf.store(b"hola").unwrap();

        let ciphertext = crypt.encrypt(3, 4, &mut buf).unwrap();
        let seq = crypt.current_seq();

        let mut corrupted = ciphertext.to_vec();
        corrupted[ciphertext.len() - 1] ^= 0xFF; // flip bit

        let mut buf2 = types::PacketBuffer::from(&corrupted[..]);
        let err = crypt
            .decrypt(seq, corrupted.len() as u16, &mut buf2)
            .unwrap_err();

        assert!(err.to_string().contains("decryption failure"), "{}", err);
    }

    #[test]
    fn test_decrypt_fails_on_truncated_ciphertext() {
        let key = SharedSecret::new([4u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        let mut buf = types::PacketBuffer::new();
        buf.store(b"hola").unwrap();

        let ciphertext = crypt.encrypt(3, 4, &mut buf).unwrap();
        let seq = crypt.current_seq();

        let truncated = ciphertext[..ciphertext.len() - 5].to_vec();

        let mut buf2 = types::PacketBuffer::from(&truncated[..]);
        let err = crypt
            .decrypt(seq, truncated.len() as u16, &mut buf2)
            .unwrap_err();

        assert!(
            err.to_string().contains("ciphertext too short"),
            "{:?}",
            err
        );
    }

    #[test]
    fn test_parse_header() {
        let mut buf = [0u8; 10];
        build_header(123, 456, &mut buf).unwrap();

        let (seq, len) = parse_header(&buf).unwrap();
        assert_eq!(seq, 123);
        assert_eq!(len, 456);
    }

    #[test]
    fn test_parse_header_invalid_size() {
        let buf = [0u8; 5];
        assert!(parse_header(&buf).is_err());
    }

    #[test]
    fn test_build_header() {
        let mut buf = [0u8; 10];
        build_header(0x1122334455667788, 0x99AA, &mut buf).unwrap();

        assert_eq!(&buf[0..8], &0x1122334455667788u64.to_be_bytes());
        assert_eq!(&buf[8..10], &0x99AAu16.to_be_bytes());
    }

    #[test]
    fn test_encrypt_does_not_overwrite_extra_bytes() {
        let key = SharedSecret::new([9u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        let mut buf = types::PacketBuffer::new();
        buf.stream_slice().fill(0xAF); // Fill with known pattern

        let before = buf.stream_slice().to_vec();

        // Channel 32, 4 bytes of data
        let _ = crypt.encrypt(32, 5, &mut buf).unwrap();

        let after = buf.stream_slice();

        // Just first 5 +  2 + 16 bytes can be changed (data + channel + tag)
        assert_eq!(&before[23..], &after[23..]);
    }
    #[test]
    fn test_encrypt_produces_unique_nonces() {
        let key = SharedSecret::new([10u8; 32]);
        let mut crypt = Crypt::new(&key, 0);

        let mut buf1 = types::PacketBuffer::new();
        buf1.store(b"a").unwrap();
        let c1 = crypt.encrypt(1, 1, &mut buf1).unwrap().to_vec();

        let mut buf2 = types::PacketBuffer::new();
        buf2.store(b"a").unwrap();
        let c2 = crypt.encrypt(1, 1, &mut buf2).unwrap().to_vec();
        assert_ne!(c1, c2);
    }
}
