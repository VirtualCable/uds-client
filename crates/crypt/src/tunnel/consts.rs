// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

pub const MAX_PACKET_SIZE: usize = 4096; // Hard limit for packet size. Anythig abobe this will be rejected.
pub const HEADER_SIZE: usize = 8 + 2; // counter (8 bytes) + length (2 bytes)
pub const TAG_LENGTH: usize = 16; // AES-GCM tag length
// IPv6 minimum MTU is 1280 bytes, minus IP (40 bytes) and UDP (8 bytes, future) headers - leaves 1232 bytes for payload
// We use 1200 + HEADER_LENGTH + TAG_LENGTH = 1226 bytes to have some margin
pub const CRYPT_PACKET_SIZE: usize = 1200; // This is our preferred packet size for encryption/decryption

// Max time once a crypt packet is started before receive it completely, to avoid hanging connections
// Its long enough to allow for slow connections, but short enough to avoid a malformed packet to keep the connection hanging indefinitely
pub const CRYPT_HANDSHAKE_TIMEOUT_SECS: u64 = 5;

pub const BUFFER_SIZE: usize = MAX_PACKET_SIZE + 2 + HEADER_SIZE; // 2 bytes for channel id, rest for data and header
pub const HEADER_START: usize = 0;
pub const CHANNEL_ID_START: usize = HEADER_SIZE;
pub const DATA_START: usize = HEADER_SIZE + 2;
