use anyhow::Result;

mod connection;
mod proxy;
mod consts;

pub struct TunnelConnectInfo {
    pub addr: String,
    pub port: u16,
    pub ticket: String,
    pub local_port: Option<u16>,  // It None, a random port will be used
    pub check_certificate: bool,  // whether to check server certificate
    pub listen_timeout_ms: u64,          // Timeout for listening
    pub keep_listening_after_timeout: bool,    // whether to keep listening after timeout
    pub enable_ipv6: bool,        // whether to enable ipv6 (local and remote)
}

// Tunnel shoud:
//   * Listen on local_port (or random port if None)
//   * If a timeout is specified, stop listening after timeout unless keep_listening_after_timeout is true
//   * If keep_listening_after_timeout is false, timeout will be 60 seconds

// For open a connection, we should
//   * Connect to addr:port
//   * Send HANDSHAKEV1, dependin on stage
//   * Upgrade output connection to SSL, with ciphers configured to use only strong ciphers
//   * SSL Should not use compression
//   * Use system certificates (so ensure flags or rustls is used correctly)
//   * Ignore certificate errors if check_certificate is false

// For a TEST connection, we should
//   * Open the conection as before
//   * Send TEST command
//   * SHOULD recieve RESPONSE_OK or an error

// For tunnel to connect, we should
//   * Open the connection as before
//   * Send OPEN command with ticket
//   * Receive exactly 2 bytes as response
//   * If response is RESPONSE_OK, start tunneling, else get 128 byte error message (at mosrt) and close the connection

// Also should:
//   * Support IPv6 if enable_ipv6 is true
//   * Listen local connections on the port, and for each connection, open a new connection to the remote server
//   * Before listening, we must test the connection to the server with a TEST command
//   * All will be threads. This is a local server and will only be a few connections, so threads are fine
//   * Read should not block (from both sides)
//   * Write should not block (from both sides)
//   * As soon as data is available on one side, it should be sent to the other side with minimal latency


pub fn start_tunnel(_info: TunnelConnectInfo) -> Result<u32> {
    // TODO: implement tunnel launching
    // Must open the tunnel on a thread, but wait for the tunne to be fully established
    Ok(0)
}

#[cfg(test)]
mod test_utils;