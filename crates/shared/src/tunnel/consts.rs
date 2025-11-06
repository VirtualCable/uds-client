#![allow(dead_code)]
pub const BUFFER_SIZE: usize = 16 * 1024; // Max buffer length
pub const LISTEN_ADDRESS: &str = "127.0.0.1";
pub const LISTEN_ADDRESS_V6: &str = "::1";
pub const RESPONSE_OK: &[u8] = b"OK";

pub const TICKET_LENGTH: usize = 48;
pub const LEGACY_TICKET_LENGTH: usize = 40;

pub const HANDSHAKE_V1: &[u8] = b"\x5AMGB\xA5\x01\x00";
pub const CMD_TEST: &[u8] = b"TEST";
pub const CMD_OPEN: &[u8] = b"OPEN";
