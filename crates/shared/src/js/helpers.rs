use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use anyhow::{Context as _, Result};
use regex::Regex;

pub(super) fn expand_vars(input: &str) -> Result<String> {
    #[cfg(target_os = "windows")]
    let re =
        Regex::new(r"%([A-Za-z0-9_]+)%").context("Failed to compile Windows variable regex")?;

    #[cfg(not(target_os = "windows"))]
    let re = Regex::new(r"\$([A-Za-z0-9_]+)|\$\{([A-Za-z0-9_]+)\}")
        .context("Failed to compile Unix variable regex")?;

    let result = re.replace_all(input, |caps: &regex::Captures| {
        #[cfg(target_os = "windows")]
        {
            let var = &caps[1];
            std::env::var(var).unwrap_or_else(|_| String::new())
        }

        #[cfg(not(target_os = "windows"))]
        {
            let var = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str())
                .unwrap_or("");
            std::env::var(var).unwrap_or_else(|_| String::new())
        }
    });

    Ok(result.into_owned())
}

pub(super) fn test_server(host: &str, port: u16, timeout_ms: u64) -> bool {
    let addr = format!("{}:{}", host, port);
    let timeout = Duration::from_millis(timeout_ms);

    match addr.to_socket_addrs() {
        Ok(mut addrs) => {
            if let Some(sockaddr) = addrs.next() {
                TcpStream::connect_timeout(&sockaddr, timeout).is_ok()
            } else {
                false
            }
        }
        Err(_) => false,
    }
}
