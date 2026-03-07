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

use super::{Crypt, types::PacketBuffer};

impl Crypt {
    // Reads data into buffer, decrypting it inplace
    // First 2 bytes are channel, rest is encrypted data + tag
    // Note: This is not cancel safe, some data may be already read on cancel.
    //       We only can use it with "stop"
    pub async fn read<'a, R: AsyncReadExt + Unpin>(
        &mut self,
        stop: &Trigger,
        reader: &mut R,
        buffer: &'a mut PacketBuffer,
    ) -> Result<(&'a [u8], u16)> {
        if tokio::select! {
            _ = stop.wait_async() => {
                log::debug!("Inbound stream stopped while reading");
                return Ok((&buffer.data()[..0], 0));  // Indicate end of stream, no error
            }
            result = buffer.read(reader) => {
                result
            }
        }? == 0 { // EOF, fine, return empty data (end of stream, no error)
            log::debug!("EOF on crypted stream");
            return Ok((&buffer.data()[..0], 0));
        }
        self.decrypt(buffer)?;
        let channel = buffer.channel_id();
        let data = buffer.data();
        Ok((data, channel))
    }

    // Writes data from buffer, encrypting it inplace
    pub async fn write<W: AsyncWriteExt + Unpin>(
        &mut self,
        stop: &Trigger,
        writer: &mut W,
        channel: u16,
        data: &[u8],
    ) -> Result<()> {
        let mut buffer = PacketBuffer::from(data);

        let length = data.len();
        self.encrypt(channel, length, &mut buffer)?;

        let result = tokio::select! {
            _ = stop.wait_async() => {
                log::debug!("Outbound stream stopped while writing");
                Ok(())  // Indicate end of processing
            }
            result = buffer.write(writer) => {
                result.map(|_| ())  // Convert to Result<()>
            }
        };
        log::debug!("WRI: seq {}, length {}, channel {}", self.seq, data.len(), channel);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SharedSecret;
    use shared::log;

    #[tokio::test]
    async fn test_read_write_roundtrip() {
        log::setup_logging("debug", log::LogType::Test);

        let stop = Trigger::new();

        let key = SharedSecret::new([7u8; 32]);
        let mut crypt = Crypt::new(&key, 0);
        // Create a pair of in-memory streams
        let (mut client, mut server) = tokio::io::duplex(1024);
        let plaintext = b"Hello, this is a test message!32";
        let mut buffer = PacketBuffer::new();

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

        assert_eq!(channel, 1);
        assert_eq!(decrypted_data, plaintext);
    }
}
