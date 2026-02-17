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

use super::consts;

// hard limited size buffer for packets
#[derive(Debug)]
pub struct PacketBuffer {
    buffer: [u8; consts::MAX_PACKET_SIZE],
}

impl PacketBuffer {
    pub fn new() -> Self {
        PacketBuffer {
            buffer: [0u8; consts::MAX_PACKET_SIZE],
        }
    }

    // Copies data into buffer, reserving first 2 bytes for channel
    pub fn from_slice(data: &[u8]) -> Self {
        let mut packet_buffer = PacketBuffer::new();
        let len = (data.len() + 2).min(consts::MAX_PACKET_SIZE);
        packet_buffer.buffer[2..len].copy_from_slice(&data[..len - 2]);
        packet_buffer
    }

    // Copies data into buffer, full buffer use (for stream use)
    pub fn from_stream_slice(data: &[u8]) -> Self {
        let mut packet_buffer = PacketBuffer::new();
        let len = data.len().min(consts::MAX_PACKET_SIZE);
        packet_buffer.buffer[..len].copy_from_slice(&data[..len]);
        packet_buffer
    }

    // Returns a mutable slice of the buffer of given length
    pub fn stream_slice(&mut self) -> &mut [u8; consts::MAX_PACKET_SIZE] {
        &mut self.buffer
    }

    pub fn ensure_capacity(&mut self, size: usize) -> Result<()> {
        if size <= consts::MAX_PACKET_SIZE {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Buffer too small: {} < {}",
                self.buffer.len(),
                size
            ))
        }
    }

    // Note: This metod is intended to get the encrypted data from buffer
    //       First two bytes are reserved for channel id
    //       So always use this to get data from buffer (it already skips first 2 bytes)
    pub fn as_slice(&self, length: usize) -> &[u8] {
        &self.buffer[2..length + 2]
    }

    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.buffer[2..]
    }

    // Note: This metod is intended to fill the buffer but leaving first 2 bytes for channel id
    //       First two bytes are reserved for channel id
    //       So always use this to copy data into buffer (it already skips first 2 bytes)
    pub fn store(&mut self, data: &[u8]) -> Result<()> {
        let len = data.len();
        self.ensure_capacity(len + 2)?;
        self.buffer[2..len + 2].copy_from_slice(data);
        Ok(())
    }
}

impl Default for PacketBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&[u8]> for PacketBuffer {
    fn from(data: &[u8]) -> Self {
        let mut packet_buffer = PacketBuffer::new();
        let len = data.len().min(consts::MAX_PACKET_SIZE);
        packet_buffer.buffer[..len].copy_from_slice(&data[..len]);
        packet_buffer
    }
}
