#![allow(dead_code)]
use anyhow::Result;

use crate::consts::TICKET_LENGTH;
use crypt::types::Ticket;

const RESERVED_LENGTH: usize = 6;

#[derive(Debug)]
pub struct OpenResponse {
    pub session_id: Ticket,
    pub channel_count: u16,
    _reserved: [u8; RESERVED_LENGTH], // For future use, 0 right now
}

impl OpenResponse {
    pub fn new(session_id: Ticket, channel_count: u16) -> Self {
        OpenResponse {
            session_id,
            channel_count,
            _reserved: [0u8; RESERVED_LENGTH],
        }
    }

    pub fn as_vec(&self) -> Vec<u8> {
        let mut vec = self.session_id.as_ref().to_vec();
        vec.extend_from_slice(&self.channel_count.to_be_bytes());
        vec.extend_from_slice(&self._reserved);
        vec
    }

    pub fn from_slice(data: &[u8]) -> Result<Self> {
        if data.len() != TICKET_LENGTH + 2 + RESERVED_LENGTH {
            anyhow::bail!("Invalid OpenResponse length");
        }
        let session_id = Ticket::try_from(&data[0..TICKET_LENGTH])?;
        let channel_count = u16::from_be_bytes(
            data[TICKET_LENGTH..TICKET_LENGTH + 2]
                .try_into()
                .map_err(|_| anyhow::anyhow!("Failed to parse channel count"))?,
        );
        //
        // let mut reserved = [0u8; RESERVED_LENGTH];
        // reserved.copy_from_slice(&data[TICKET_LENGTH + 2..]);
        Ok(OpenResponse::new(session_id, channel_count))
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
        let open_response = OpenResponse::new(session_id, channel_count);
        let vec = open_response.as_vec();
        let parsed = OpenResponse::try_from(vec.as_slice()).expect("Failed to parse OpenResponse");
        assert_eq!(parsed.session_id, session_id);
        assert_eq!(parsed.channel_count, channel_count);
    }

    #[test]
    fn test_open_response_invalid_length() {
        let data = vec![0u8; TICKET_LENGTH + 1]; // Invalid length
        let result = OpenResponse::try_from(data.as_slice());
        assert!(result.is_err());
    }

    #[test]
    fn test_open_response_invalid_channel_count() {
        let session_id = Ticket::new([1u8; TICKET_LENGTH]);
        let mut vec = session_id.as_ref().to_vec();
        vec.extend_from_slice(&[0xFF, 0xFF]); // Invalid channel count (65535)
        vec.extend_from_slice(&[0u8; RESERVED_LENGTH]);
        let result = OpenResponse::try_from(vec.as_slice());
        assert!(result.is_ok()); // Channel count is valid, just large
        let open_response = result.unwrap();
        assert_eq!(open_response.channel_count, 65535);
    }
}
