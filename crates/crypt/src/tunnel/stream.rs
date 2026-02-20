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

// Authors: Adolfo Gómez, dkmaster at dkmon dot compub mod broker;

use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use shared::{log, system::trigger::Trigger};

use super::{
    Crypt, build_header, consts::HEADER_LENGTH, parse_header,
    types::PacketBuffer,
};

impl Crypt {
    async fn read_stream<R: AsyncReadExt + Unpin>(
        stop: &Trigger,
        reader: &mut R,
        buffer: &mut [u8],
        length: usize,
        disallow_eof: bool,
    ) -> Result<usize> {
        let mut read = 0;

        while read < length {
            let n = tokio::select! {
                _ = stop.wait_async() => {
                    log::debug!("Inbound stream stopped while reading");
                    return Ok(0);  // Indicate end of processing
                }
                result = reader.read(&mut buffer[read..length]) => {
                    match result {
                        Ok(0) => {
                            if disallow_eof || read != 0 {
                                return Err(anyhow::anyhow!("connection closed unexpectedly"));
                            } else {
                                return Ok(0);  // Connection closed
                            }
                        }
                        Ok(n) => n,
                        Err(e) => {
                            return Err(anyhow::format_err!("read error: {:?}", e));
                        }
                    }
                }
            };
            read += n;
        }
        Ok(read)
    }

    // Reads data into buffer, decrypting it inplace
    // First 2 bytes are channel, rest is encrypted data + tag
    pub async fn read<'a, R: AsyncReadExt + Unpin>(
        &mut self,
        stop: &Trigger,
        reader: &mut R,
        buffer: &'a mut PacketBuffer,
    ) -> Result<(&'a [u8], u16)> {
        let mut header_buffer: [u8; HEADER_LENGTH] = [0; HEADER_LENGTH];
        if Self::read_stream(stop, reader, header_buffer.as_mut(), HEADER_LENGTH, false).await? == 0
        {
            // Connection closed
            return Ok((&buffer.as_slice_mut()[..0], 0)); // Empty vector indicates closed connection, ensures has 'a lifetime
        }
        // Check valid header and get payload length
        let (seq, length) = parse_header(&header_buffer[..HEADER_LENGTH])?;
        // Read the encrypted payload + tag
        if Self::read_stream(stop, reader, buffer.stream_slice(), length as usize, true).await? == 0
        {
            // Connection closed
            log::error!("Inbound stream closed while reading payload");
            return Err(anyhow::anyhow!(
                "connection closed unexpectedly while reading payload"
            ));
        }
        self.decrypt(seq, length, buffer)
    }

    // Writes data from buffer, encrypting it inplace
    pub async fn write<W: AsyncWriteExt + Unpin>(
        &mut self,
        stop: &Trigger,
        writer: &mut W,
        channel: u16,
        data: &[u8],
    ) -> Result<()> {
        let mut header_buffer: [u8; HEADER_LENGTH] = [0; HEADER_LENGTH];
        let length = data.len();
        let mut buff = PacketBuffer::new();
        buff.store(data)?;
        let encrypted_packet = self.encrypt(channel, length, &mut buff)?;
        build_header(
            self.current_seq(),
            encrypted_packet.len() as u16,
            &mut header_buffer,
        )?;
        tokio::select! {
            _ = stop.wait_async() => {
                log::debug!("Outbound stream stopped while writing");
                Ok(())  // Indicate end of processing
            }
            result = async {
                writer.write_all(&header_buffer).await?;
                writer.write_all(encrypted_packet).await?;
                Ok(())
            } => {
                result
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SharedSecret;

    #[tokio::test]
    async fn test_read_write_roundtrip() {
        log::setup_logging("debug", log::LogType::Test);

        let key = SharedSecret::new([7u8; 32]);
        let mut crypt = Crypt::new(&key, 0);
        // Create a pair of in-memory streams
        let (mut client, mut server) = tokio::io::duplex(1024);
        let plaintext = b"Hello, this is a test message!32";
        let mut buffer = PacketBuffer::new();

        let stop = Trigger::new();

        // Write data from client to server
        crypt
            .write(&stop, &mut client, 1, plaintext)
            .await
            .expect("Failed to write data");

        // Read data from server to client
        let (decrypted_data, channel) = crypt
            .read(&stop, &mut server, &mut buffer)
            .await
            .expect("Failed to read data");

        log::debug!("Decrypted data: {:?}", decrypted_data);

        assert_eq!(channel, 1);
        assert_eq!(decrypted_data, plaintext);
    }
}
