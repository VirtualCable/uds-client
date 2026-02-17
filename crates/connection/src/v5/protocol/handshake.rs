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
use num_enum::{FromPrimitive, IntoPrimitive};
use tokio::io::AsyncWriteExt;

use super::consts::HANDSHAKE_V2_SIGNATURE;
use crypt::types::Ticket;

// Handshake commands, starting from 0
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum HandshakeCommand {
    Test = 0,
    Open = 1,
    Recover = 2,
    #[num_enum(default)]
    Unknown = 255,
}

// Posible handshakes:
//   - With or without PROXY protocol v2 header
//   - HANDSHAKE_V2 | cmd:u8 | payload_cmd_dependent
//        Test | no payload
//        Open | ticket[48] | ticket encrpyted with HKDF-derived key  --> returns session id for new session
//        Recover | ticket[48] | ticket encrypted with HKDF-derived key (this ticket is the session id of the lost session) -> returns same as Open (new session id)
//   - Full handshake should occur on at most 0.2 seconds
//   - Any failed handhsake, closes without response (hide server presence as much as possible)
//   - TODO: Make some kind of block by IP if too many failed handshakes in short time

pub enum Handshake {
    Test,
    Open { ticket: Ticket },
    Recover { ticket: Ticket },
}

impl From<&Handshake> for Vec<u8> {
    fn from(action: &Handshake) -> Self {
        match action {
            Handshake::Test => vec![HandshakeCommand::Test.into()],
            Handshake::Open { ticket } => {
                let mut buf = Vec::new();
                buf.push(HandshakeCommand::Open.into());
                buf.extend_from_slice(ticket.as_ref());
                buf
            }
            Handshake::Recover { ticket } => {
                let mut buf = Vec::new();
                buf.push(HandshakeCommand::Recover.into());
                buf.extend_from_slice(ticket.as_ref());
                buf
            }
        }
    }
}

impl Handshake {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(HANDSHAKE_V2_SIGNATURE);
        buf.extend_from_slice(&Vec::from(self));
        buf
    }

    pub async fn write<W: tokio::io::AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.to_bytes()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use shared::log;

    use super::super::consts::TICKET_LENGTH;

    #[test]
    fn test_handshake_open_to_bytes() {
        log::setup_logging("debug", log::LogType::Test);
        let ticket = Ticket::new_random();
        let handshake = Handshake::Open { ticket };
        let bytes = handshake.to_bytes();
        assert!(bytes.starts_with(HANDSHAKE_V2_SIGNATURE));
        assert_eq!(
            bytes[HANDSHAKE_V2_SIGNATURE.len()],
            u8::from(HandshakeCommand::Open)
        );
        assert_eq!(
            bytes.len(),
            HANDSHAKE_V2_SIGNATURE.len() + 1 + TICKET_LENGTH
        );
        assert_eq!(&bytes[HANDSHAKE_V2_SIGNATURE.len() + 1..], ticket.as_ref());
    }

    #[test]
    fn test_handshake_recover_to_bytes() {
        log::setup_logging("debug", log::LogType::Test);
        let ticket = Ticket::new_random();
        let handshake = Handshake::Recover { ticket };
        let bytes = handshake.to_bytes();
        assert!(bytes.starts_with(HANDSHAKE_V2_SIGNATURE));
        assert_eq!(
            bytes[HANDSHAKE_V2_SIGNATURE.len()],
            u8::from(HandshakeCommand::Recover)
        );

        assert_eq!(
            bytes.len(),
            HANDSHAKE_V2_SIGNATURE.len() + 1 + TICKET_LENGTH
        );
        assert_eq!(&bytes[HANDSHAKE_V2_SIGNATURE.len() + 1..], ticket.as_ref());
    }

    #[test]
    fn test_handshake_test_to_bytes() {
        log::setup_logging("debug", log::LogType::Test);
        let handshake = Handshake::Test;
        let bytes = handshake.to_bytes();
        assert!(bytes.starts_with(HANDSHAKE_V2_SIGNATURE));
        assert_eq!(
            bytes[HANDSHAKE_V2_SIGNATURE.len()],
            u8::from(HandshakeCommand::Test)
        );
        assert_eq!(bytes.len(), HANDSHAKE_V2_SIGNATURE.len() + 1);
    }
}
