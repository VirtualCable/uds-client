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
use rand::{distr::Alphanumeric, prelude::*};

use shared::utils::hex_to_bytes;

use super::consts::TICKET_LENGTH;

// Hard type for shared secret
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SharedSecret([u8; 32]);

/// This code block is implementing functionality for the `SharedSecret` struct in Rust. Here's a
/// breakdown of what each part is doing:
impl SharedSecret {
    pub fn new(secret: [u8; 32]) -> Self {
        SharedSecret(secret)
    }

    pub fn from_hex(hex_str: &str) -> Result<Self> {
        let bytes = hex_to_bytes::<32>(hex_str)?;
        Ok(SharedSecret(bytes))
    }
}

impl AsRef<[u8; 32]> for SharedSecret {
    fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<[u8; 32]> for SharedSecret {
    fn from(secret: [u8; 32]) -> Self {
        SharedSecret(secret)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ticket([u8; TICKET_LENGTH]);

impl Ticket {
    pub fn new(id: [u8; TICKET_LENGTH]) -> Self {
        Ticket(id)
    }

    pub fn new_random() -> Self {
        let rng = rand::rng();
        let id = rng
            .sample_iter(Alphanumeric)
            .take(TICKET_LENGTH)
            .collect::<Vec<u8>>()
            .try_into()
            .expect("Failed to create Ticket");
        Self(id)
    }

    pub fn validate(&self) -> Result<()> {
        if !self.0.iter().all(|&c| c.is_ascii_alphanumeric()) {
            return Err(anyhow::anyhow!("Invalid ticket"));
        }
        Ok(())
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).expect("Ticket is not valid UTF-8")
    }
}

impl AsRef<[u8; TICKET_LENGTH]> for Ticket {
    fn as_ref(&self) -> &[u8; TICKET_LENGTH] {
        &self.0
    }
}

impl From<[u8; TICKET_LENGTH]> for Ticket {
    fn from(id: [u8; TICKET_LENGTH]) -> Self {
        Ticket::new(id)
    }
}

impl From<&[u8; TICKET_LENGTH]> for Ticket {
    fn from(id: &[u8; TICKET_LENGTH]) -> Self {
        Ticket::new(*id)
    }
}

impl TryFrom<&[u8]> for Ticket {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Ticket> {
        if value.len() != TICKET_LENGTH {
            return Err(anyhow::anyhow!("Invalid ticket length"));
        }
        let mut id = [0u8; TICKET_LENGTH];
        id.copy_from_slice(value);
        Ok(Ticket::new(id))
    }
}
