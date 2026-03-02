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
        let length = self.length().unwrap_or(consts::MAX_PACKET_SIZE).saturating_sub(2);
        &mut self.buffer[consts::DATA_START..consts::DATA_START + length]
    }

    pub fn data(&self) -> &[u8] {
        let length = self.length().unwrap_or(consts::MAX_PACKET_SIZE).saturating_sub(2);
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
