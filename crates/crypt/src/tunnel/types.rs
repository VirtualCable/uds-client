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
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::consts;

// hard limited size buffer for packets
#[derive(Clone)]
pub struct PacketBuffer {
    // Buffer has 3 parts:
    // - First HEADER_SIZE bytes for header (used in stream mode)
    // - Next 2 bytes for channel id (used in packet mode)
    // - Rest for data (buffer)
    buffer: [u8; consts::BUFFER_SIZE],
}

impl PacketBuffer {
    pub fn new() -> Self {
        PacketBuffer {
            buffer: [0u8; consts::BUFFER_SIZE],
        }
    }

    pub fn set_channel_id(&mut self, channel_id: u16) {
        self.buffer[consts::CHANNEL_ID_START..consts::CHANNEL_ID_START + 2]
            .copy_from_slice(&channel_id.to_be_bytes());
    }

    pub fn channel_id(&self) -> u16 {
        u16::from_be_bytes(
            self.buffer[consts::CHANNEL_ID_START..consts::CHANNEL_ID_START + 2]
                .try_into()
                .unwrap(),
        )
    }

    // Copies data in
    pub fn set_data(&mut self, data: &[u8]) -> Result<()> {
        let len = data.len();
        Self::ensure_capacity(len + consts::DATA_START)?;
        self.buffer[consts::DATA_START..len + consts::DATA_START].copy_from_slice(data);
        Ok(())
    }

    // length is length of data + channel id (2 bytes)
    pub fn data_mut(&mut self) -> &mut [u8] {
        let length = self
            .length()
            .unwrap_or(consts::MAX_PACKET_SIZE)
            .saturating_sub(2);
        &mut self.buffer[consts::DATA_START..consts::DATA_START + length]
    }

    pub fn data(&self) -> &[u8] {
        let length = self
            .length()
            .unwrap_or(consts::MAX_PACKET_SIZE)
            .saturating_sub(2);
        &self.buffer[consts::DATA_START..consts::DATA_START + length]
    }

    pub fn data_with_channel_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[consts::CHANNEL_ID_START..]
    }

    pub fn data_with_channel(&self) -> &[u8] {
        &self.buffer[consts::CHANNEL_ID_START..]
    }

    pub fn buffer(&self) -> Result<&[u8]> {
        let length = self.length()?;
        Ok(&self.buffer[..length + consts::HEADER_SIZE])
    }

    pub fn buffer_mut(&mut self) -> Result<&mut [u8]> {
        let length = self.length()?;
        Ok(&mut self.buffer[..length + consts::HEADER_SIZE])
    }

    pub fn full_buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buffer
    }

    pub fn seq(&self) -> Result<u64> {
        Ok(u64::from_be_bytes(
            self.buffer[consts::HEADER_START..consts::HEADER_START + 8]
                .try_into()
                .map_err(|_| anyhow::anyhow!("invalid header for seq"))?,
        ))
    }

    pub fn set_seq(&mut self, seq: u64) {
        self.buffer[consts::HEADER_START..consts::HEADER_START + 8]
            .copy_from_slice(&seq.to_be_bytes());
    }

    pub fn length(&self) -> Result<usize> {
        Ok(u16::from_be_bytes(
            self.buffer[consts::HEADER_START + 8..consts::HEADER_START + 10]
                .try_into()
                .map_err(|_| anyhow::anyhow!("invalid header for length"))?,
        ) as usize)
    }

    pub fn set_length(&mut self, length: usize) -> Result<()> {
        if length > consts::MAX_PACKET_SIZE {
            return Err(anyhow::anyhow!("invalid packet length: {}", length));
        }
        let length = length as u16;
        self.buffer[consts::HEADER_START + 8..consts::HEADER_START + 10]
            .copy_from_slice(&length.to_be_bytes());
        Ok(())
    }

    pub fn header_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[consts::HEADER_START..consts::HEADER_START + consts::HEADER_SIZE]
    }

    pub fn header(&self) -> &[u8] {
        &self.buffer[consts::HEADER_START..consts::HEADER_START + consts::HEADER_SIZE]
    }

    pub fn validate_header(&self) -> Result<usize> {
        // Just check if length is valid, seq can be any value
        let length = self.length()?;
        if !(2..consts::MAX_PACKET_SIZE).contains(&length) {
            return Err(anyhow::anyhow!(
                "invalid packet length in header: {}",
                length
            ));
        }
        Ok(length)
    }

    pub async fn write<W: AsyncWriteExt + Unpin>(&self, writer: &mut W) -> Result<()> {
        // len = header + channel id + data len (in header)
        let total_len = consts::HEADER_SIZE + self.length()?;
        writer
            .write_all(&self.buffer[0..total_len])
            .await
            .map_err(|e| anyhow::anyhow!("write error: {:?}", e))
    }

    async fn read_stream<R: AsyncReadExt + Unpin>(
        reader: &mut R,
        buffer: &mut [u8],
        length: usize,
        allow_eof: bool,
    ) -> Result<usize> {
        let mut read = 0;

        while read < length {
            let n = match reader.read(&mut buffer[read..length]).await {
                Ok(0) => {
                    if !allow_eof || read != 0 {
                        return Err(anyhow::anyhow!("connection closed unexpectedly"));
                    } else {
                        return Ok(0); // Connection closed
                    }
                }
                Ok(n) => n,
                Err(e) => {
                    return Err(anyhow::format_err!("read error: {:?}", e));
                }
            };
            read += n;
        }
        Ok(read)
    }

    // Not cancel safe, take care when using it (only with "ending alternate branches"?)
    pub async fn read<R: AsyncReadExt + Unpin>(&mut self, reader: &mut R) -> Result<usize> {
        // Read first header
        if Self::read_stream(reader, self.header_mut(), consts::HEADER_SIZE, true).await? == 0 {
            return Ok(0); // Connection closed
        }
        let length = self.validate_header()?;
        Self::read_stream(
            reader,
            &mut self.buffer[consts::HEADER_SIZE..consts::HEADER_SIZE + length],
            length,
            false,
        )
        .await?;
        // Note: channel_id + is not decrypted here (handle it elsewhere if needed)
        Ok(consts::HEADER_SIZE + length)
    }

    pub fn ensure_capacity(size: usize) -> Result<()> {
        if size <= consts::MAX_PACKET_SIZE - consts::DATA_START {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Buffer too small: {} < {}",
                consts::MAX_PACKET_SIZE - consts::DATA_START,
                size,
            ))
        }
    }

    pub fn calc_data_with_channel_len(length: usize) -> Result<usize> {
        Self::ensure_capacity(length + consts::CHANNEL_ID_START)?;
        Ok(length + 2)
    }

    pub fn create(seq: u64, length: usize, channel_id: u16, data: &[u8]) -> Result<Self> {
        let mut packet_buffer = PacketBuffer::new();
        packet_buffer.set_seq(seq);
        packet_buffer.set_length(length)?;
        packet_buffer.set_channel_id(channel_id);
        packet_buffer.set_data(data)?;
        Ok(packet_buffer)
    }
}

impl Default for PacketBuffer {
    fn default() -> Self {
        Self::new()
    }
}

// Debug, showing info about channel and data length, but not data itself (can be too big)
impl std::fmt::Debug for PacketBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PacketBuffer")
            .field("seq", &self.seq().unwrap_or(0))
            .field("length", &self.length().unwrap_or(0))
            .field("data_length", &self.data().len())
            .finish()
    }
}

impl From<&[u8]> for PacketBuffer {
    fn from(data: &[u8]) -> Self {
        let mut packet_buffer = PacketBuffer::new();
        packet_buffer
            .set_data(data)
            .expect("Data too large for buffer");
        packet_buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_header_min_valid() {
        let mut pb = PacketBuffer::new();
        pb.set_length(2).unwrap(); // channel only, no data
        assert_eq!(pb.validate_header().unwrap(), 2);
    }

    #[test]
    fn validate_header_zero_fails() {
        // set_length(0) succeeds, but validate_header rejects length < 2
        let mut pb = PacketBuffer::new();
        pb.set_length(0).unwrap();
        assert!(pb.validate_header().is_err());
    }

    #[test]
    fn validate_header_length_one_fails() {
        let mut pb = PacketBuffer::new();
        pb.set_length(1).unwrap();
        assert!(pb.validate_header().is_err());
    }

    #[test]
    fn validate_header_too_big_fails() {
        let mut pb = PacketBuffer::new();
        assert!(pb.set_length(consts::MAX_PACKET_SIZE + 1).is_err());
    }

    #[test]
    fn set_length_boundary() {
        let mut pb = PacketBuffer::new();
        assert!(pb.set_length(consts::MAX_PACKET_SIZE).is_ok());
        assert_eq!(pb.length().unwrap(), consts::MAX_PACKET_SIZE);
    }

    #[test]
    fn set_length_zero_ok_but_validation_fails() {
        // set_length(0) doesn't err, but validate_header rejects it
        let mut pb = PacketBuffer::new();
        assert!(pb.set_length(0).is_ok());
        assert!(pb.validate_header().is_err());
    }

    #[test]
    fn create_factory_roundtrip() {
        let data = b"hello, tunnel!";
        let pb = PacketBuffer::create(42, data.len() + 2, 7, data).unwrap();
        assert_eq!(pb.seq().unwrap(), 42);
        assert_eq!(pb.length().unwrap(), data.len() + 2);
        assert_eq!(pb.channel_id(), 7);
        assert_eq!(pb.data(), data);
    }

    #[test]
    fn create_data_too_large() {
        let data = vec![0u8; consts::MAX_PACKET_SIZE];
        assert!(PacketBuffer::create(0, data.len() + 2, 0, &data).is_err());
    }

    #[test]
    fn ensure_capacity_valid() {
        assert!(PacketBuffer::ensure_capacity(0).is_ok());
        assert!(PacketBuffer::ensure_capacity(2048).is_ok());
    }

    #[test]
    fn ensure_capacity_too_large() {
        assert!(PacketBuffer::ensure_capacity(10_000).is_err());
    }

    #[test]
    fn calc_data_with_channel_len_basic() {
        assert_eq!(PacketBuffer::calc_data_with_channel_len(0).unwrap(), 2);
        assert_eq!(PacketBuffer::calc_data_with_channel_len(100).unwrap(), 102);
    }

    #[test]
    fn calc_data_with_channel_len_boundary() {
        assert!(PacketBuffer::calc_data_with_channel_len(1000).is_ok());
        // 10_000 is way beyond BUFFER_SIZE
        assert!(PacketBuffer::calc_data_with_channel_len(10_000).is_err());
    }

    #[test]
    fn set_channel_id_roundtrip() {
        let mut pb = PacketBuffer::new();
        for id in [0u16, 0x1234, u16::MAX] {
            pb.set_channel_id(id);
            assert_eq!(pb.channel_id(), id);
        }
    }

    #[test]
    fn set_data_roundtrip() {
        let mut pb = PacketBuffer::new();
        pb.set_data(b"test").unwrap();
        pb.set_length(2 + 4).unwrap();
        assert_eq!(pb.data(), b"test");
    }
}
