use std::ops::Deref;
use std::ops::DerefMut;

use anyhow::Result;

mod command;
pub mod consts;
pub mod handshake;
pub mod ticket;

pub use command::Command;

#[derive(Debug, Clone)]
pub struct Payload(pub Vec<u8>);

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

#[derive(Debug, Clone)]
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
