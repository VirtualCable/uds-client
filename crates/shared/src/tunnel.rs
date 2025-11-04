use anyhow::Result;

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


pub fn start_tunnel(_info: TunnelConnectInfo) -> Result<u32> {
    // TODO: implement tunnel launching
    Ok(0)
}