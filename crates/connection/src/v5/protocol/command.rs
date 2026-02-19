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

// All commands goes on channel 0
use anyhow::Result;
use num_enum::{FromPrimitive, IntoPrimitive};

use super::{PayloadWithChannel, consts::MAX_ERROR_MSG_LENGTH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
enum CommandType {
    // Generic OK response
    //   - From client to tunnel: not used
    //   - From tunnel to client: means "last command was successful"
    //   Used in reponse to OpenChannel and CloseChannel (Close does not get any response)
    Ok = 0,
    // OpenChannel:
    //   - From client to tunnel, means "open this channel"
    //   - From tunnel to client: not used
    OpenChannel,
    // CloseChannel:
    //   - From client to tunnel, means "close this channel"
    //   - From tunnel to client, means "the channel is CORRECTLY closed"
    CloseChannel,
    // Close:
    //   - From client to tunnel, means "close the connection"
    //   - From tunnel to client: not used
    //     for example, due to error, or service restart
    Close, // Close connection
    // ChannelError:
    //   - From client to tunnel, not used
    //   - From tunnel to client, means "an error happened on channel"
    //     The channel is shutdown on tunnel.
    ChannelError,
    // ConnectionError:
    //   - From client to tunnel, not used
    //   - From tunnel to client, means "an error happened on connection", should not happpen, so this may be considered fatal?
    ConnectionError,
    // NOP: Used to skip a packet, for example, on a out of order packet, or a se keep-alive
    //   - From client to tunnel, means "this packet is a NOP, ignore it"
    //   - From tunnel to client, means "this packet is a NOP, ignore
    Nop,

    // Unknown command:
    //    - Just a placeholder for unknown commands. Will cause a ConnectionError ALWAYS
    #[num_enum(default)]
    Unknown = 255,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Ok,
    OpenChannel { channel_id: u16 },
    CloseChannel { channel_id: u16 },
    Close,
    ChannelError { channel_id: u16, message: String },
    ConnectionError { message: String },
    Nop,
}

// The command comes inside the encrypted data part of a packet as this:
//   channel_id:u16 | command_type:u8 | command_dependent_payload
// channeld_id is always 0, as channel 0 is reserved for control (commands)
// Channel is already stripped when calling this function, so data contains only:
//   command_type:u8 | command_dependent_payload
impl Command {
    pub fn from_slice(data: &[u8]) -> Result<Command> {
        if data.is_empty() {
            anyhow::bail!("command data too short");
        }
        let cmd_type: CommandType = data[0].into();
        match cmd_type {
            CommandType::Ok => Ok(Command::Ok),
            CommandType::OpenChannel => {
                if data.len() < 3 {
                    anyhow::bail!("OpenChannel command data too short");
                }
                let channel_id = u16::from_be_bytes([data[1], data[2]]);
                Ok(Command::OpenChannel { channel_id })
            }
            CommandType::CloseChannel => {
                if data.len() < 3 {
                    anyhow::bail!("CloseChannel command data too short");
                }
                let channel_id = u16::from_be_bytes([data[1], data[2]]);
                Ok(Command::CloseChannel { channel_id })
            }
            CommandType::Close => Ok(Command::Close),
            CommandType::ChannelError => {
                if data.len() < 3 {
                    anyhow::bail!("ChannelError command data too short");
                }
                let channel_id = u16::from_be_bytes([data[1], data[2]]);
                let message =
                    String::from_utf8_lossy(&data[3..3 + MAX_ERROR_MSG_LENGTH.min(data.len() - 3)])
                        .to_string();
                Ok(Command::ChannelError {
                    channel_id,
                    message,
                })
            }
            CommandType::ConnectionError => {
                let message =
                    String::from_utf8_lossy(&data[1..1 + MAX_ERROR_MSG_LENGTH.min(data.len() - 1)])
                        .to_string();
                Ok(Command::ConnectionError { message })
            }
            CommandType::Nop => Ok(Command::Nop),
            CommandType::Unknown => {
                anyhow::bail!("Unknown command received");
            }
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        match self {
            Command::Ok => {
                data.push(CommandType::Ok.into());
            }
            Command::OpenChannel { channel_id } => {
                data.push(CommandType::OpenChannel.into());
                data.extend_from_slice(&channel_id.to_be_bytes());
            }
            Command::CloseChannel { channel_id } => {
                data.push(CommandType::CloseChannel.into());
                data.extend_from_slice(&channel_id.to_be_bytes());
            }
            Command::Close => {
                data.push(CommandType::Close.into());
            }
            Command::ChannelError {
                channel_id,
                message,
            } => {
                data.push(CommandType::ChannelError.into());
                data.extend_from_slice(&channel_id.to_be_bytes());
                data.extend_from_slice(message.as_bytes()[..MAX_ERROR_MSG_LENGTH.min(message.len())].as_ref());
            }
            Command::ConnectionError { message } => {
                data.push(CommandType::ConnectionError.into());
                data.extend_from_slice(message.as_bytes()[..MAX_ERROR_MSG_LENGTH.min(message.len())].as_ref());
            }
            Command::Nop => {
                data.push(CommandType::Nop.into());
            }
        }
        data
    }

    pub fn to_message(&self) -> PayloadWithChannel {
        PayloadWithChannel::new(0, self.to_bytes().as_slice()) // channel 0 is reserved for commands
    }

    pub fn is_close_command(&self) -> bool {
        matches!(
            self,
            Command::CloseChannel { .. }
                | Command::ConnectionError { .. }
                | Command::ChannelError { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_open_channel() {
        let cmd = Command::OpenChannel { channel_id: 42 };
        let bytes = cmd.to_bytes();
        let parsed = Command::from_slice(&bytes).unwrap();
        assert_eq!(parsed, cmd);
    }

    #[test]
    fn open_channel_too_short() {
        let data = [CommandType::OpenChannel as u8, 0x01]; // just 2 bytes
        assert!(Command::from_slice(&data).is_err());
    }

    #[test]
    fn channel_error_too_short() {
        let data = [CommandType::ChannelError as u8, 0x00];
        assert!(Command::from_slice(&data).is_err());
    }

    #[test]
    fn empty_data_fails() {
        assert!(Command::from_slice(&[]).is_err());
    }

    #[test]
    fn unknown_command_fails() {
        let data = [255u8];
        assert!(Command::from_slice(&data).is_err());
    }

    #[test]
    fn channel_error_invalid_utf8() {
        let data = [
            CommandType::ChannelError as u8,
            0x00,
            0x01,
            0xFF,
            0xFE,
            0xFD,
        ];
        let cmd = Command::from_slice(&data).unwrap();
        if let Command::ChannelError { message, .. } = cmd {
            assert!(message.contains("�")); // replacement char
        } else {
            panic!("expected ChannelError");
        }
    }

    #[test]
    fn connection_error_empty_message() {
        let data = [CommandType::ConnectionError as u8];
        let cmd = Command::from_slice(&data).unwrap();
        assert_eq!(cmd, Command::ConnectionError { message: "".into() });
    }

    #[test]
    fn to_bytes_close_channel() {
        let cmd = Command::CloseChannel { channel_id: 0x1234 };
        let bytes = cmd.to_bytes();
        assert_eq!(bytes, vec![CommandType::CloseChannel as u8, 0x12, 0x34,]);
    }

    #[test]
    fn long_channel_error_message() {
        let msg = "a".repeat(500);
        let cmd = Command::ChannelError {
            channel_id: 1,
            message: msg.clone(),
        };
        let bytes = cmd.to_bytes();
        let parsed = Command::from_slice(&bytes).unwrap();

        if let Command::ChannelError { message, .. } = parsed {
            assert_eq!(message, msg);
        } else {
            panic!("expected ChannelError");
        }
    }
}
