// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

#![allow(dead_code)]
use std::time::Duration;
pub const BUFFER_SIZE: usize = 16 * 1024; // Max buffer length
pub const RESPONSE_OK: &[u8] = b"OK";

pub const TICKET_LENGTH: usize = 48;

pub const HANDSHAKE_V1: &[u8] = b"\x5AMGB\xA5\x01\x00";

pub const CMD_TEST: &[u8] = b"TEST";
pub const CMD_OPEN: &[u8] = b"OPEN";
pub const CMD_LENGTH: usize = 4;

// Max. time for commands to complete. This is a big value to account for slow networks.
pub const CMD_TIMEOUT_SECS: Duration = Duration::from_secs(4);
