// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::Result;

use aes_gcm::{
    AeadInOut, Aes256Gcm, Nonce, Tag,
    aead::{AeadCore, KeyInit},
};

use crate::{tunnel::consts, types::SharedSecret};

pub mod replay;

use replay::ReplayWindow;

/// Length of the per-session UDP token, in bytes.
pub const TOKEN_LENGTH: usize = 16;
/// Per-session identifier assigned by the tunnel server so its shared UDP
/// socket can demultiplex datagrams to their owning session. A zero token
/// means "UDP disabled".
pub type UdpToken = [u8; TOKEN_LENGTH];

/// Cleartext datagram header: token (16 bytes) + seq (8 bytes, big-endian).
pub const DATAGRAM_HEADER_SIZE: usize = TOKEN_LENGTH + 8;
/// Maximum payload carried by one tunnel datagram. Sized so that a full
/// RDPUDP2 datagram (MTU 1232) fits.
///
/// MTU note: the worst-case wire datagram is 1272 bytes of UDP payload
/// (24 header + 1232 payload + 16 tag), i.e. 1300/1320 bytes on the wire
/// with IPv4/IPv6 headers. That is fine on typical 1500-byte paths, but it
/// exceeds the IPv6 *guaranteed minimum* MTU budget (1280 - 48 = 1232) by
/// exactly our 40-byte overhead: on a path with MTU < 1320, max-size
/// datagrams would IP-fragment or drop. Accepted trade-off: RDPUDP absorbs
/// that as ordinary loss and retransmits, and shrinking the cap below 1232
/// would break full-size RDPUDP2 datagrams on *every* path.
pub const MAX_DATAGRAM_PAYLOAD: usize = consts::CRYPT_PACKET_SIZE + 32;
/// Minimum valid datagram: header + tag + at least one payload byte.
const MIN_DATAGRAM_SIZE: usize = DATAGRAM_HEADER_SIZE + consts::TAG_LENGTH + 1;
const MAX_DATAGRAM_SIZE: usize = DATAGRAM_HEADER_SIZE + MAX_DATAGRAM_PAYLOAD + consts::TAG_LENGTH;

/// Initial value of the UDP send sequence counter.
///
/// Starts at 2^63 so UDP datagram seqs are unmistakable from the TCP leg
/// counters (which start at 0/1) when inspecting traffic, and so that the
/// 64-bit counter can never wrap: 2^63 remaining datagrams at a sustained
/// 1M datagrams/second would take ~292,000 years.
pub const INITIAL_SEQ: u64 = 1 << 63;

/// AEAD crypt for the UDP leg of the tunnel.
///
/// Same AES-256-GCM construction as the stream `Crypt` (nonce = seq padded to
/// 12 bytes), but with ordering semantics relaxed for datagrams: the receiver
/// tracks seen sequence numbers with a sliding `ReplayWindow` instead of
/// requiring strictly increasing values, and AAD binds the session token so a
/// datagram cannot be replayed across sessions.
///
/// Keys and sequence numbers are fully independent of the TCP leg. There is
/// no retransmission and no reordering: what is lost is lost (RDPUDP handles
/// reliability end to end).
pub struct DatagramCrypt {
    cipher: Aes256Gcm,
    send_seq: u64,
    window: ReplayWindow,
}

impl DatagramCrypt {
    pub fn new(key: &SharedSecret) -> Self {
        DatagramCrypt {
            cipher: Aes256Gcm::new(key.as_ref().into()),
            send_seq: INITIAL_SEQ,
            window: ReplayWindow::new(),
        }
    }

    /// Current send sequence (last value used; next datagram gets send_seq + 1).
    pub fn current_seq(&self) -> u64 {
        self.send_seq
    }

    fn nonce_for(seq: u64) -> Nonce<<Aes256Gcm as AeadCore>::NonceSize> {
        let mut nonce_arr = [0u8; 12];
        nonce_arr[..8].copy_from_slice(&seq.to_be_bytes());
        nonce_arr.into()
    }

    fn aad_for(token: &UdpToken, seq: u64) -> [u8; TOKEN_LENGTH + 8] {
        let mut aad = [0u8; TOKEN_LENGTH + 8];
        aad[..TOKEN_LENGTH].copy_from_slice(token);
        aad[TOKEN_LENGTH..].copy_from_slice(&seq.to_be_bytes());
        aad
    }

    /// Encrypts one payload into a full wire datagram: token | seq | ct | tag.
    pub fn encrypt(&mut self, token: &UdpToken, payload: &[u8]) -> Result<Vec<u8>> {
        if payload.is_empty() || payload.len() > MAX_DATAGRAM_PAYLOAD {
            return Err(anyhow::anyhow!(
                "invalid datagram payload size: {}",
                payload.len()
            ));
        }

        self.send_seq += 1;
        let seq = self.send_seq;
        let nonce = Self::nonce_for(seq);
        let aad = Self::aad_for(token, seq);

        let mut data = payload.to_vec();
        let tag = self
            .cipher
            .encrypt_inout_detached(&nonce, &aad, data.as_mut_slice().into())
            .map_err(|e| anyhow::anyhow!("datagram encryption failure: {:?}", e))?;

        let mut out = Vec::with_capacity(DATAGRAM_HEADER_SIZE + data.len() + consts::TAG_LENGTH);
        out.extend_from_slice(token);
        out.extend_from_slice(&seq.to_be_bytes());
        out.append(&mut data);
        out.extend_from_slice(tag.as_slice());
        Ok(out)
    }

    /// Decrypts one wire datagram for the given session token.
    ///
    /// - `Ok(Some(payload))` — authentic, fresh datagram.
    /// - `Ok(None)` — benign discard: wrong token, duplicate, or too old for
    ///   the replay window. Normal on UDP, not an error.
    /// - `Err` — malformed or failed AEAD verification (forgery attempt).
    ///
    /// The replay window is only marked after a successful AEAD verification,
    /// so a forged datagram with a huge seq cannot poison the window.
    pub fn decrypt(&mut self, token: &UdpToken, datagram: &[u8]) -> Result<Option<Vec<u8>>> {
        if datagram.len() < MIN_DATAGRAM_SIZE || datagram.len() > MAX_DATAGRAM_SIZE {
            return Err(anyhow::anyhow!("invalid datagram size: {}", datagram.len()));
        }
        if &datagram[..TOKEN_LENGTH] != token {
            return Ok(None); // Not for this session; benign on a shared socket
        }

        let seq = u64::from_be_bytes(
            datagram[TOKEN_LENGTH..DATAGRAM_HEADER_SIZE]
                .try_into()
                .expect("slice length checked above"),
        );
        let nonce = Self::nonce_for(seq);
        let aad = Self::aad_for(token, seq);

        let ct_len = datagram.len() - DATAGRAM_HEADER_SIZE - consts::TAG_LENGTH;
        let mut data = datagram[DATAGRAM_HEADER_SIZE..DATAGRAM_HEADER_SIZE + ct_len].to_vec();
        let tag: &Tag = (&datagram[datagram.len() - consts::TAG_LENGTH..])
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid datagram tag length"))?;

        self.cipher
            .decrypt_inout_detached(&nonce, &aad, data.as_mut_slice().into(), tag)
            .map_err(|e| anyhow::anyhow!("datagram decryption failure: {:?}", e))?;

        if !self.window.check_and_mark(seq) {
            return Ok(None); // Duplicate or too old
        }

        Ok(Some(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_token() -> UdpToken {
        [0x42u8; TOKEN_LENGTH]
    }

    fn crypt_pair() -> (DatagramCrypt, DatagramCrypt) {
        let key = SharedSecret::new([7u8; 32]);
        (DatagramCrypt::new(&key), DatagramCrypt::new(&key))
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let (mut sender, mut receiver) = crypt_pair();
        let token = test_token();
        let payload = b"rdp-udp-payload";

        let datagram = sender.encrypt(&token, payload).unwrap();
        assert_eq!(
            datagram.len(),
            DATAGRAM_HEADER_SIZE + payload.len() + consts::TAG_LENGTH
        );

        let decrypted = receiver.decrypt(&token, &datagram).unwrap();
        assert_eq!(decrypted.as_deref(), Some(payload.as_slice()));
    }

    #[test]
    fn test_out_of_order_and_loss_are_accepted() {
        let (mut sender, mut receiver) = crypt_pair();
        let token = test_token();

        let d1 = sender.encrypt(&token, b"one").unwrap();
        let _d2 = sender.encrypt(&token, b"two").unwrap(); // simulates loss: never delivered
        let d3 = sender.encrypt(&token, b"three").unwrap();
        let d4 = sender.encrypt(&token, b"four").unwrap();

        // Loss of d2, reordering of d3/d4: all fine
        assert_eq!(
            receiver.decrypt(&token, &d4).unwrap().as_deref(),
            Some(b"four".as_slice())
        );
        assert_eq!(
            receiver.decrypt(&token, &d3).unwrap().as_deref(),
            Some(b"three".as_slice())
        );
        assert_eq!(
            receiver.decrypt(&token, &d1).unwrap().as_deref(),
            Some(b"one".as_slice())
        );
        // d2 never arrived; nothing breaks
        let d5 = sender.encrypt(&token, b"five").unwrap();
        assert_eq!(
            receiver.decrypt(&token, &d5).unwrap().as_deref(),
            Some(b"five".as_slice())
        );
    }

    #[test]
    fn test_duplicate_is_discarded_not_error() {
        let (mut sender, mut receiver) = crypt_pair();
        let token = test_token();

        let d1 = sender.encrypt(&token, b"data").unwrap();
        assert!(receiver.decrypt(&token, &d1).unwrap().is_some());
        assert!(receiver.decrypt(&token, &d1).unwrap().is_none());
    }

    #[test]
    fn test_wrong_token_is_discarded_not_error() {
        let (mut sender, mut receiver) = crypt_pair();
        let token = test_token();
        let other_token = [0x99u8; TOKEN_LENGTH];

        let d1 = sender.encrypt(&token, b"data").unwrap();
        assert!(receiver.decrypt(&other_token, &d1).unwrap().is_none());
        // And the datagram is still valid for its real session afterwards
        assert!(receiver.decrypt(&token, &d1).unwrap().is_some());
    }

    #[test]
    fn test_tampered_datagram_fails() {
        let (mut sender, mut receiver) = crypt_pair();
        let token = test_token();

        let mut d1 = sender.encrypt(&token, b"data").unwrap();
        let last = d1.len() - 1;
        d1[last] ^= 0xFF; // flip a bit in the tag
        assert!(receiver.decrypt(&token, &d1).is_err());
    }

    #[test]
    fn test_forged_huge_seq_does_not_poison_window() {
        let (mut sender, mut receiver) = crypt_pair();
        let token = test_token();

        // Attacker without the key forges a datagram with a huge seq.
        // AEAD must reject it BEFORE the window is touched.
        let mut forged = Vec::new();
        forged.extend_from_slice(&token);
        forged.extend_from_slice(&(u64::MAX - 1).to_be_bytes());
        forged.extend_from_slice(&[0u8; 8]); // fake ciphertext
        forged.extend_from_slice(&[0u8; consts::TAG_LENGTH]);
        assert!(receiver.decrypt(&token, &forged).is_err());

        // Legitimate traffic keeps flowing: window was not advanced
        let d1 = sender.encrypt(&token, b"legit").unwrap();
        assert!(receiver.decrypt(&token, &d1).unwrap().is_some());
    }

    #[test]
    fn test_truncated_and_oversized_datagrams_fail() {
        let (mut sender, mut receiver) = crypt_pair();
        let token = test_token();

        assert!(receiver.decrypt(&token, &[0u8; 10]).is_err());
        let huge = vec![0u8; MAX_DATAGRAM_SIZE + 1];
        assert!(receiver.decrypt(&token, &huge).is_err());

        // Max payload roundtrips
        let payload = vec![0xCDu8; MAX_DATAGRAM_PAYLOAD];
        let d = sender.encrypt(&token, &payload).unwrap();
        assert_eq!(
            receiver.decrypt(&token, &d).unwrap().as_deref(),
            Some(payload.as_slice())
        );
    }

    #[test]
    fn test_empty_payload_rejected() {
        let (mut sender, _) = crypt_pair();
        assert!(sender.encrypt(&test_token(), b"").is_err());
    }

    #[test]
    fn test_seq_increments_and_is_unique() {
        let (mut sender, _) = crypt_pair();
        let token = test_token();

        let d1 = sender.encrypt(&token, b"a").unwrap();
        let d2 = sender.encrypt(&token, b"a").unwrap();
        assert_ne!(d1, d2);
        assert_eq!(sender.current_seq(), INITIAL_SEQ + 2);
        assert_eq!(
            &d1[TOKEN_LENGTH..DATAGRAM_HEADER_SIZE],
            &(INITIAL_SEQ + 1).to_be_bytes()
        );
        assert_eq!(
            &d2[TOKEN_LENGTH..DATAGRAM_HEADER_SIZE],
            &(INITIAL_SEQ + 2).to_be_bytes()
        );
    }
}
