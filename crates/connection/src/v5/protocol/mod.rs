// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use core::fmt;
use std::ops::Deref;
use std::ops::DerefMut;

use anyhow::Result;

mod command;
pub mod consts;
pub mod handshake;

pub use command::Command;

#[derive(Debug, Clone)]
pub struct Payload(pub Vec<u8>);

impl Payload {
    pub fn new(data: &[u8]) -> Self {
        Payload(data.to_vec())
    }
}

impl From<Vec<u8>> for Payload {
    fn from(value: Vec<u8>) -> Self {
        Payload(value)
    }
}

impl<const N: usize> From<&[u8; N]> for Payload {
    fn from(value: &[u8; N]) -> Self {
        Payload(value.to_vec())
    }
}

impl From<&[u8]> for Payload {
    fn from(value: &[u8]) -> Self {
        Payload(value.to_vec())
    }
}

impl AsRef<[u8]> for Payload {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Deref for Payload {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Payload {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Payload {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Clone)]
pub struct PayloadWithChannel {
    pub channel_id: u16,
    pub payload: Payload,
}

impl PayloadWithChannel {
    pub fn new(channel_id: u16, payload: &[u8]) -> Self {
        PayloadWithChannel {
            channel_id,
            payload: payload.into(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 2 {
            anyhow::bail!("Message too short to contain channel_id");
        }
        let channel_id = u16::from_be_bytes([bytes[0], bytes[1]]);
        let payload = bytes[2..].to_vec();
        Ok(PayloadWithChannel {
            channel_id,
            payload: payload.into(),
        })
    }

    pub fn len(&self) -> usize {
        self.payload.len() + 2 // Include channel_id bytes
    }

    pub fn is_empty(&self) -> bool {
        self.payload.is_empty()
    }
}

impl fmt::Debug for PayloadWithChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PayloadWithChannel")
            .field("chan", &self.channel_id)
            .field("len", &self.payload.len())
            .finish()
    }
}

// Channel types
pub type PayloadSender = flume::Sender<Payload>;
pub type PayloadReceiver = flume::Receiver<Payload>;
pub type PayloadWithChannelSender = flume::Sender<PayloadWithChannel>;
pub type PayloadWithChannelReceiver = flume::Receiver<PayloadWithChannel>;

pub fn payload_pair() -> (PayloadSender, PayloadReceiver) {
    flume::bounded(consts::CHANNEL_SIZE)
}

pub fn payload_with_channel_pair() -> (PayloadWithChannelSender, PayloadWithChannelReceiver) {
    let (tx, rx) = flume::bounded(consts::CHANNEL_SIZE);
    (tx, rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bytes_valid() {
        let p = PayloadWithChannel::from_bytes(&[0x00, 0x01, 0xAA, 0xBB]).unwrap();
        assert_eq!(p.channel_id, 1);
        assert_eq!(&p.payload[..], &[0xAA, 0xBB]);
    }

    #[test]
    fn from_bytes_empty() {
        assert!(PayloadWithChannel::from_bytes(&[]).is_err());
    }

    #[test]
    fn from_bytes_too_short() {
        assert!(PayloadWithChannel::from_bytes(&[0x00]).is_err());
    }

    #[test]
    fn from_bytes_min_channel() {
        let p = PayloadWithChannel::from_bytes(&[0x00, 0x00]).unwrap();
        assert_eq!(p.channel_id, 0);
        assert!(p.payload.is_empty());
    }

    #[test]
    fn from_bytes_max_channel() {
        let p = PayloadWithChannel::from_bytes(&[0xFF, 0xFF, 0x42]).unwrap();
        assert_eq!(p.channel_id, 65535);
        assert_eq!(&p.payload[..], &[0x42]);
    }

    #[test]
    fn len_includes_channel_header() {
        let p = PayloadWithChannel::new(0, &[1, 2, 3]);
        assert_eq!(p.len(), 5); // 3 + 2
    }

    #[test]
    fn is_empty_delegates_to_payload() {
        let p = PayloadWithChannel::new(0, &[]);
        assert!(p.is_empty());
        let p = PayloadWithChannel::new(0, &[1]);
        assert!(!p.is_empty());
    }
}
