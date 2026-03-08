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
//
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
