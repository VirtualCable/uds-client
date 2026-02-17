// Handshake constants
pub const HANDSHAKE_V2_SIGNATURE: &[u8; 8] = b"\x5AMGB\xA5\x02\x00\x00";
pub const HANDSHAKE_TIMEOUT_MS: u64 = 200;
pub const MAX_HANDSHAKE_RETRIES: u8 = 3; // After this, we can block the IP for a while

// Proxy protocol v2 constants
pub const PROXY_V2_SIGNATURE: [u8; 12] = [
    0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A,
];

// Channel related constants
pub const CHANNEL_SIZE: usize = 2048; // 2k messages as much on a channel buffer

// Ticket related constants
pub const TICKET_LENGTH: usize = 48;
