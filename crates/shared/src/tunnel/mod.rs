use anyhow::Result;

mod connection;
mod consts;
mod proxy;

pub struct TunnelConnectInfo {
    pub addr: String,
    pub port: u16,
    pub ticket: String,
    pub local_port: Option<u16>, // It None, a random port will be used
    pub check_certificate: bool, // whether to check server certificate
    pub listen_timeout_ms: u64,  // Timeout for listening
    pub keep_listening_after_timeout: bool, // whether to keep listening after timeout
    pub enable_ipv6: bool,       // whether to enable ipv6 (local and remote)
}

pub async fn start_tunnel(info: TunnelConnectInfo) -> Result<u32> {
    let tls_stream =
        connection::connect_and_upgrade(&info.addr, info.port, info.check_certificate).await?;
    let (mut reader, mut writer) = tokio::io::split(tls_stream);

    // Test to ensure connection is valid
    connection::test_connection(&mut reader, &mut writer).await?;

    connection::open_connection(&mut reader, &mut writer, &info.ticket).await?;

    // TODO: Start proxying data between local port and tls_stream
    // Ok((reader, writer))

    Ok(0) // Placeholder
}

#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod tests;
