// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

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
        }? == 0
        {
            // EOF, fine, return empty data (end of stream, no error)
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
