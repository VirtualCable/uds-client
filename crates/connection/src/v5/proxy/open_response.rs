#![allow(dead_code)]

// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use anyhow::Result;

use crate::consts::TICKET_LENGTH;
use crypt::datagram::{TOKEN_LENGTH, UdpToken};
use crypt::types::Ticket;

const RESERVED_LENGTH: usize = 6;

// Layout: session_id(48) | channel_count(2) | inbound_seq(8) | outbound_seq(8)
//       | udp_token(16) | udp_port(2) | reserved(6) = 90 bytes
const OPEN_RESPONSE_LENGTH: usize =
    TICKET_LENGTH + 2 + 8 + 8 + TOKEN_LENGTH + 2 + RESERVED_LENGTH;

const UDP_TOKEN_START: usize = TICKET_LENGTH + 2 + 8 + 8;
const UDP_PORT_START: usize = UDP_TOKEN_START + TOKEN_LENGTH;

// Important Note:
// inbound is inbound for REMOTE tunnel (so, our outbound),
//and outbound is outbound for REMOTE tunnel (so, our inbound)
#[derive(Debug)]
pub struct OpenResponse {
    pub session_id: Ticket,
    pub channel_count: u16,
    pub inbound_seq: u64,
    pub outbound_seq: u64,
    pub udp_token: UdpToken, // All zero means UDP is disabled for this session
    pub udp_port: u16, // Resolved UDP relay port on the server; 0 means "same as the TCP tunnel port"
    _reserved: [u8; RESERVED_LENGTH], // For future use, 0 right now
}

impl OpenResponse {
    pub fn new(
        session_id: Ticket,
        channel_count: u16,
        inbound_seq: u64,
        outbound_seq: u64,
        udp_token: UdpToken,
        udp_port: u16,
    ) -> Self {
        OpenResponse {
            session_id,
            channel_count,
            inbound_seq,
            outbound_seq,
            udp_token,
            udp_port,
            _reserved: [0u8; RESERVED_LENGTH],
        }
    }

    pub fn udp_enabled(&self) -> bool {
        self.udp_token != [0u8; TOKEN_LENGTH]
    }

    pub fn as_vec(&self) -> Vec<u8> {
        let mut vec = self.session_id.as_ref().to_vec();
        vec.extend_from_slice(&self.channel_count.to_be_bytes());
        vec.extend_from_slice(&self.inbound_seq.to_be_bytes());
        vec.extend_from_slice(&self.outbound_seq.to_be_bytes());
        vec.extend_from_slice(&self.udp_token);
        vec.extend_from_slice(&self.udp_port.to_be_bytes());
        vec.extend_from_slice(&self._reserved);
        vec
    }

    pub fn from_slice(data: &[u8]) -> Result<Self> {
        if data.len() != OPEN_RESPONSE_LENGTH {
            anyhow::bail!(
                "Invalid OpenResponse length: expected {}, got {}",
                OPEN_RESPONSE_LENGTH,
                data.len()
            );
        }
        let session_id = Ticket::try_from(&data[0..TICKET_LENGTH])?;
        let channel_count = u16::from_be_bytes(
            data[TICKET_LENGTH..TICKET_LENGTH + 2]
                .try_into()
                .map_err(|_| anyhow::anyhow!("Failed to parse channel count"))?,
        );
        let inbound_seq = u64::from_be_bytes(
            data[TICKET_LENGTH + 2..TICKET_LENGTH + 2 + 8]
                .try_into()
                .map_err(|_| anyhow::anyhow!("Failed to parse inbound sequence"))?,
        );
        let outbound_seq = u64::from_be_bytes(
            data[TICKET_LENGTH + 2 + 8..TICKET_LENGTH + 2 + 8 + 8]
                .try_into()
                .map_err(|_| anyhow::anyhow!("Failed to parse outbound sequence"))?,
        );
        let udp_token: UdpToken = data[UDP_TOKEN_START..UDP_TOKEN_START + TOKEN_LENGTH]
            .try_into()
            .map_err(|_| anyhow::anyhow!("Failed to parse UDP token"))?;
        let udp_port = u16::from_be_bytes(
            data[UDP_PORT_START..UDP_PORT_START + 2]
                .try_into()
                .map_err(|_| anyhow::anyhow!("Failed to parse UDP port"))?,
        );
        Ok(OpenResponse::new(
            session_id,
            channel_count,
            inbound_seq,
            outbound_seq,
            udp_token,
            udp_port,
        ))
    }
}

impl TryFrom<&[u8]> for OpenResponse {
    type Error = anyhow::Error;

    fn try_from(data: &[u8]) -> Result<Self, Self::Error> {
        OpenResponse::from_slice(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_response_serialization() {
        let session_id = Ticket::new([1u8; TICKET_LENGTH]);
        let channel_count = 1;
        let udp_token = [0xABu8; TOKEN_LENGTH];
        let udp_port = 4443u16;
        let open_response =
            OpenResponse::new(session_id.clone(), channel_count, 1, 2, udp_token, udp_port);
        let vec = open_response.as_vec();
        assert_eq!(vec.len(), OPEN_RESPONSE_LENGTH);
        assert_eq!(OPEN_RESPONSE_LENGTH, 90);
        let parsed = OpenResponse::try_from(vec.as_slice()).expect("Failed to parse OpenResponse");
        assert_eq!(parsed.session_id, session_id);
        assert_eq!(parsed.channel_count, channel_count);
        assert_eq!(parsed.inbound_seq, 1);
        assert_eq!(parsed.outbound_seq, 2);
        assert_eq!(parsed.udp_token, udp_token);
        assert_eq!(parsed.udp_port, udp_port);
        assert!(parsed.udp_enabled());
    }

    #[test]
    fn test_open_response_raw_offsets() {
        let session_id = Ticket::new([1u8; TICKET_LENGTH]);
        let udp_token = [0x42u8; TOKEN_LENGTH];
        let vec = OpenResponse::new(session_id, 0x0102, 1, 2, udp_token, 0x0506).as_vec();
        assert_eq!(vec.len(), 90);
        // channel_count at 48
        assert_eq!(&vec[48..50], &[0x01, 0x02]);
        // udp_token at 66..82
        assert_eq!(&vec[66..82], &[0x42u8; TOKEN_LENGTH]);
        // udp_port at 82..84, big-endian
        assert_eq!(&vec[82..84], &[0x05, 0x06]);
        // reserved 84..90 must be zero
        assert!(vec[84..90].iter().all(|&b| b == 0));
    }

    #[test]
    fn test_open_response_zero_token_means_udp_disabled() {
        let session_id = Ticket::new([1u8; TICKET_LENGTH]);
        let open_response = OpenResponse::new(session_id, 1, 1, 1, [0u8; TOKEN_LENGTH], 0);
        let parsed = OpenResponse::try_from(open_response.as_vec().as_slice()).unwrap();
        assert_eq!(parsed.udp_token, [0u8; TOKEN_LENGTH]);
        assert_eq!(parsed.udp_port, 0);
        assert!(!parsed.udp_enabled());
    }

    #[test]
    fn test_open_response_invalid_length() {
        let data = vec![0u8; TICKET_LENGTH + 1]; // Invalid length
        let result = OpenResponse::try_from(data.as_slice());
        assert!(result.is_err());

        // Old 72-byte layout (pre-UDP) must be rejected
        let old_layout = vec![0u8; TICKET_LENGTH + 2 + 8 + 8 + RESERVED_LENGTH];
        assert!(OpenResponse::try_from(old_layout.as_slice()).is_err());

        // Old 88-byte layout (token but no udp_port) must be rejected
        let old_layout = vec![0u8; TICKET_LENGTH + 2 + 8 + 8 + TOKEN_LENGTH + RESERVED_LENGTH];
        assert!(OpenResponse::try_from(old_layout.as_slice()).is_err());
    }

    #[test]
    fn test_open_response_invalid_channel_count() {
        let session_id = Ticket::new([1u8; TICKET_LENGTH]);
        let mut vec = session_id.as_ref().to_vec();
        vec.extend_from_slice(&[0xFF, 0xFF]); // Invalid channel count (65535)
        vec.extend_from_slice(&[0u8; 8]); // Inbound seq
        vec.extend_from_slice(&[0u8; 8]); // Outbound seq
        vec.extend_from_slice(&[0u8; TOKEN_LENGTH]); // UDP token (disabled)
        vec.extend_from_slice(&[0u8; 2]); // UDP port
        vec.extend_from_slice(&[0u8; RESERVED_LENGTH]);
        let result = OpenResponse::try_from(vec.as_slice());
        assert!(result.is_ok()); // Channel count is valid, just large
        let open_response = result.unwrap();
        assert_eq!(open_response.channel_count, 65535);
    }
}
