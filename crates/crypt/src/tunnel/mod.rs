// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::Arc;

use anyhow::Result;

use aes_gcm::{AeadInOut, Aes256Gcm, Nonce, Tag, aead::KeyInit};

use shared::log;

use crate::rekey::{DIR_LAUNCHER_TO_SERVER, RekeyState, TRANSPORT_TCP};
use crate::types::SharedSecret;

// Comms related
pub mod consts;
pub mod stream;
pub mod types;

/// AES-GCM crypt for one TCP tunnel direction, with deterministic per-seq
/// rekeying (mirror of the tunnel-server's `shared::crypt::Crypt`).
///
/// The launcher builds the handshake crypt pair from the legacy epoch-0
/// material (`Crypt::new`), then installs the session's negotiated threshold
/// with [`Crypt::set_rekey`] right after parsing the `OpenResponse` and
/// before any data frame flows. Every frame whose epoch differs from the
/// crypt's cached cipher re-derives that epoch's key from the frame's own
/// sequence number; `k = 0` states collapse to a single epoch forever,
/// byte-identical to the pre-rekeying wire format.
pub struct Crypt {
    key: SharedSecret,
    /// Cipher for `cipher_epoch`, the epoch owning the last frame this crypt
    /// encrypted or authenticated. An `Arc` so an epoch crossing is a
    /// re-derive (once per `2^k` frames) and staying inside it is cheap.
    cipher: Arc<Aes256Gcm>,
    seq: u64,
    rekey: Arc<RekeyState>,
    cipher_epoch: u64,
}

impl Crypt {
    pub fn new(key: &SharedSecret, seq: u64) -> Self {
        log::debug!("Creating Crypt with initial seq: {}", seq);
        let cipher = Arc::new(Aes256Gcm::new(key.as_ref().into()));
        Crypt {
            key: key.clone(),
            cipher_epoch: 0,
            rekey: Arc::new(RekeyState::epoch0_only(
                TRANSPORT_TCP,
                DIR_LAUNCHER_TO_SERVER,
                0,
                cipher.clone(),
            )),
            cipher,
            seq,
        }
    }

    /// Creates a crypt that rekeys per `rekey`, starting on its epoch-0
    /// cipher. Used by tests and by any future caller that knows `k` up
    /// front; the production launcher path is `new` + [`Self::set_rekey`].
    pub fn with_rekey(key: &SharedSecret, seq: u64, rekey: Arc<RekeyState>) -> Self {
        Crypt {
            key: key.clone(),
            cipher_epoch: 0,
            cipher: rekey.cipher_for(0),
            rekey,
            seq,
        }
    }

    /// Attaches (or replaces) the rekeying state mid-life. The crypt must be
    /// sitting in epoch 0 at the call site — which is exactly what the
    /// launcher does after the handshake and before any data frame flows.
    /// The epoch-0 cipher is taken from the new state (a byte-identical
    /// rebuild of the same legacy key), so nothing on the wire changes at
    /// the seam.
    pub fn set_rekey(&mut self, rekey: Arc<RekeyState>) {
        self.cipher_epoch = 0;
        self.cipher = rekey.cipher_for(0);
        self.rekey = rekey;
    }

    /// Selects the cipher for `seq`, re-deriving (once per epoch crossing)
    /// when the frame belongs to a different epoch than the crypt's cached
    /// cipher. `k = 0` never leaves epoch 0, which keeps OFF byte-identical
    /// to the legacy construction.
    fn cipher_for_seq(&mut self, seq: u64) -> Arc<Aes256Gcm> {
        let epoch = self.rekey.epoch_of(seq);
        if epoch != self.cipher_epoch {
            self.cipher = self.rekey.cipher_for(epoch);
            self.cipher_epoch = epoch;
        }
        self.cipher.clone()
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

        let mut nonce_arr = [0u8; 12];
        nonce_arr[..8].copy_from_slice(&seq.to_be_bytes());
        let nonce = Nonce::from(nonce_arr);
        let aad = seq.to_be_bytes();

        // Get pointer to data part of the buffer, where encryption will happen
        let data = buffer.data_with_channel_mut();

        // log::debug!(
        //     "ENC: seq {}, length {}: {:?}..{:?}, channel {}",
        //     seq,
        //     len,
        //     data[..std::cmp::min(8, len)].to_vec(),
        //     data[len.saturating_sub(8)..data_with_channel_length].to_vec(),
        //     channel_id
        // );

        // Epoch-owned cipher: identical to `self.cipher` for every frame of
        // the current epoch, re-derived once at the epoch crossing.
        let cipher = self.cipher_for_seq(seq);
        let tag = cipher
            .encrypt_inout_detached(&nonce, &aad, (&mut data[..data_with_channel_length]).into())
            .map_err(|e| anyhow::anyhow!("encryption failure: {:?}", e))?;
        data[data_with_channel_length..data_with_channel_length + consts::TAG_LENGTH]
            .copy_from_slice(tag.as_slice());

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
        // Mirror of the server-side guard: `seq` arrives from the wire, and
        // advancing to `seq + 1` below would overflow on `u64::MAX`. No
        // honest peer ever reaches this value, so reject the frame instead of
        // panicking (debug) or wedging the counter (release).
        if seq == u64::MAX {
            return Err(anyhow::anyhow!("invalid sequence number: u64::MAX"));
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

        let mut nonce_arr = [0u8; 12];
        nonce_arr[..8].copy_from_slice(&seq.to_be_bytes());
        let nonce = Nonce::from(nonce_arr);
        let aad = seq.to_be_bytes();

        // Split ciphertext and tag. `Tag` is parameterised by the tag size (not by
        // the cipher); its default `U16` matches the tag size of `Aes256Gcm`.
        let (ciphertext, rest) = chan_data_buffer.split_at_mut(len);
        let tag: &Tag = (&rest[..consts::TAG_LENGTH])
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid tag length"))?;

        // Epoch-owned cipher. The seq comes from the frame header, so a
        // late frame of an earlier epoch (legitimately still in flight)
        // re-derives that epoch's key deterministically; the anti-replay
        // check above already rejected replays.
        let cipher = self.cipher_for_seq(seq);
        cipher
            .decrypt_inout_detached(&nonce, &aad, ciphertext.into(), tag)
            .map_err(|e| anyhow::anyhow!("decryption failure: {:?}", e))?;

        self.seq = seq + 1; // Update to last used seq + 1, so no replays are possible

        // Fix data length to remove ending tag, so only channel + data is left
        buffer.set_length(len)?;

        // let data = buffer.data_with_channel();
        // log::debug!(
        //     "DEC: seq {}, length {}: {:?}..{:?}, channel {}",
        //     seq,
        //     len,
        //     data[..std::cmp::min(8, len)].to_vec(),
        //     data[len.saturating_sub(8)..len].to_vec(),
        //     buffer.channel_id()
        // );
        Ok(())
    }
}

impl Clone for Crypt {
    fn clone(&self) -> Self {
        log::debug!("Cloning Crypt with seq: {}", self.seq);
        Crypt {
            cipher: self.cipher.clone(),
            key: self.key.clone(),
            seq: self.seq,
            rekey: self.rekey.clone(),
            cipher_epoch: self.cipher_epoch,
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
    use crate::types::{SharedSecret, Ticket};

    use super::*;
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[test]
    fn test_send_sync() {
        assert_send::<Crypt>();
        assert_sync::<Crypt>();
        assert_send::<RekeyState>();
        assert_sync::<RekeyState>();
    }

    fn rekey_state(
        secret: &SharedSecret,
        ticket: &Ticket,
        k: u8,
        epoch0: &SharedSecret,
    ) -> Arc<RekeyState> {
        Arc::new(RekeyState::new(
            Arc::new(crate::rekey::SessionPrk::derive(secret, ticket)),
            TRANSPORT_TCP,
            DIR_LAUNCHER_TO_SERVER,
            k,
            0,
            Arc::new(Aes256Gcm::new(epoch0.as_ref().into())),
        ))
    }

    /// A crypt whose rekey state was installed after the handshake rotates
    /// its cipher at the epoch boundary (mirror of the server-side test),
    /// and a frame of the previous epoch still in flight decrypts fine:
    /// the key is a pure function of the frame's own seq.
    #[test]
    fn test_rekey_across_epoch_boundary_roundtrip() {
        let secret = SharedSecret::new([0x11u8; 32]);
        let ticket = Ticket::new([0x22u8; 48]);
        let epoch0 = SharedSecret::new([0x33u8; 32]);

        // k = 2: epochs switch every 4 frames.
        let mut sender = Crypt::new(&epoch0, 0);
        let mut receiver = Crypt::new(&epoch0, 0);
        sender.set_rekey(rekey_state(&secret, &ticket, 2, &epoch0));
        receiver.set_rekey(rekey_state(&secret, &ticket, 2, &epoch0));

        // Frames 1..=12 span epochs 0, 1, 2 (seq>>2): all roundtrip.
        for i in 1..=12u64 {
            let payload = format!("frame-{i}");
            let mut buf = types::PacketBuffer::new();
            buf.set_data(payload.as_bytes()).unwrap();
            sender.encrypt(5, payload.len(), &mut buf).unwrap();
            assert_eq!(buf.seq().unwrap(), i, "seq at {i}");
            let mut buf2 = buf.clone();
            receiver.decrypt(&mut buf2).unwrap();
            assert_eq!(buf2.data(), payload.as_bytes(), "roundtrip at {i}");
        }
        assert_eq!(receiver.current_seq(), 13);

        // A late frame of an *earlier* epoch (seq 5: epoch 1, while the
        // sender sits at epoch 2) still decrypts on a receiver that has not
        // seen it yet: the key comes from the frame's seq, not from a
        // transition clock.
        let mut stale = Crypt::new(&epoch0, 4); // next encrypt gets seq 5
        stale.set_rekey(rekey_state(&secret, &ticket, 2, &epoch0));
        let mut late = types::PacketBuffer::new();
        late.set_data(b"late").unwrap();
        stale.encrypt(5, 4, &mut late).unwrap();
        assert_eq!(late.seq().unwrap(), 5);
        let mut fresh = Crypt::new(&epoch0, 0);
        fresh.set_rekey(rekey_state(&secret, &ticket, 2, &epoch0));
        fresh.decrypt(&mut late).unwrap();
        assert_eq!(late.data(), b"late");
    }

    /// With `k = 0` (OFF) — the state `Crypt::new` installs and the state
    /// `set_rekey` installs for an OFF session — the crypt must be
    /// byte-identical to the legacy construction forever.
    #[test]
    fn test_off_rekey_is_byte_identical_to_legacy() {
        let key = SharedSecret::new([0x44u8; 32]);
        let mut legacy = Crypt::new(&key, 0);

        let mut off = Crypt::new(&key, 0);
        off.set_rekey(Arc::new(RekeyState::epoch0_only(
            TRANSPORT_TCP,
            DIR_LAUNCHER_TO_SERVER,
            0,
            Arc::new(Aes256Gcm::new(key.as_ref().into())),
        )));

        for i in 0..8u16 {
            let mut a = types::PacketBuffer::new();
            let mut b = types::PacketBuffer::new();
            a.set_data(b"payload bytes").unwrap();
            b.set_data(b"payload bytes").unwrap();
            legacy.encrypt(i, 13, &mut a).unwrap();
            off.encrypt(i, 13, &mut b).unwrap();
            assert_eq!(
                a.buffer().unwrap(),
                b.buffer().unwrap(),
                "frame {i}: OFF must equal legacy"
            );
        }
    }

    /// The handshake seam: installing the rekey state with the session's `k`
    /// after the OpenResponse must not change the epoch-0 wire bytes — the
    /// seam between "legacy handshake frames" and "rekeying data frames" is
    /// invisible on the wire.
    #[test]
    fn test_set_rekey_keeps_epoch0_wire_identical() {
        let secret = SharedSecret::new([0x11u8; 32]);
        let ticket = Ticket::new([0x22u8; 48]);
        let epoch0 = SharedSecret::new([0x33u8; 32]);

        // Two crypts at the same seq: one still legacy (handshake), one with
        // the negotiated state installed (post-OpenResponse). For every
        // epoch-0 frame (with k = 8 that is seqs up to 255) they must emit
        // byte-identical wire bytes.
        for seed in [0u64, 1, 2, 254] {
            let mut before = Crypt::new(&epoch0, seed);
            let mut after = Crypt::new(&epoch0, seed);
            after.set_rekey(rekey_state(&secret, &ticket, 8, &epoch0));

            let mut a = types::PacketBuffer::new();
            let mut b = types::PacketBuffer::new();
            a.set_data(b"ticket-bytes").unwrap();
            b.set_data(b"ticket-bytes").unwrap();
            before.encrypt(0, 12, &mut a).unwrap();
            after.encrypt(0, 12, &mut b).unwrap();
            assert_eq!(
                a.buffer().unwrap(),
                b.buffer().unwrap(),
                "seam frame at seq {}",
                seed + 1
            );
        }
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

    // ── Header parse / build

    #[test]
    fn parse_header_valid() {
        let mut buf = [0u8; 10];
        buf[0..8].copy_from_slice(&42u64.to_be_bytes());
        buf[8..10].copy_from_slice(&100u16.to_be_bytes());
        let (seq, len) = parse_header(&buf).unwrap();
        assert_eq!(seq, 42);
        assert_eq!(len, 100);
    }

    #[test]
    fn parse_header_too_short() {
        assert!(parse_header(&[0u8; 9]).is_err());
        assert!(parse_header(&[]).is_err());
    }

    #[test]
    fn parse_header_length_too_big() {
        let mut buf = [0u8; 10];
        buf[8..10].copy_from_slice(&(consts::MAX_PACKET_SIZE as u16 + 1).to_be_bytes());
        assert!(parse_header(&buf).is_err());
    }

    #[test]
    fn build_header_roundtrip() {
        let mut buf = [0u8; 16];
        build_header(42, 256u16, &mut buf).unwrap();
        let (seq, len) = parse_header(&buf).unwrap();
        assert_eq!(seq, 42);
        assert_eq!(len, 256);
    }

    #[test]
    fn build_header_too_short() {
        let mut buf = [0u8; 9];
        assert!(build_header(0, 0, &mut buf).is_err());
    }

    #[test]
    fn build_header_does_not_clobber_past_10() {
        let mut buf = [0xAAu8; 20];
        build_header(1, 100, &mut buf).unwrap();
        // Bytes 10..20 should be untouched
        assert!(buf[10..].iter().all(|&b| b == 0xAA));
    }
}
