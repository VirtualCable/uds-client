use std::{fs::File, io::Write, net::ToSocketAddrs, time::Duration};
use tokio::{net::TcpStream, time::timeout};

use rand::{Rng, distr::Alphabetic};

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

pub(super) async fn test_server(host: &str, port: u16, timeout_ms: u64) -> bool {
    let addr = format!("{}:{}", host, port);
    let timeout_dur = Duration::from_millis(timeout_ms);

    match addr.to_socket_addrs() {
        Ok(mut addrs) => {
            if let Some(sockaddr) = addrs.next() {
                match timeout(timeout_dur, TcpStream::connect(sockaddr)).await {
                    Ok(Ok(_stream)) => true,  // Connection successful
                    _ => false,
                }
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

// File related helpers
pub(super) fn create_temp_file(
    folder: Option<&str>,
    content: Option<&str>,
    extension: Option<&str>,
) -> Result<String> {
    // Create a random filename with the given extension on temp dir or specified folder
    // Tries to create the folder
    let folder = if let Some(folder_path) = folder {
        std::fs::create_dir_all(folder_path)
            .with_context(|| format!("Failed to create directory: {}", folder_path))?;
        std::path::PathBuf::from(folder_path)
    } else {
        std::env::temp_dir()
    };
    // extension should not contain dot
    let extension = extension.map(|ext| ext.trim_start_matches('.'));
    // Try 3 times to avoid collisions
    for _ in 0..3 {
        let tmp_filename = folder.join(format!(
            "tmp_file_{}.{}",
            rand::rng()
                .sample_iter(&Alphabetic)
                .take(10)
                .map(char::from)
                .collect::<String>(),
            extension.unwrap_or("tmp")
        ));
        let mut file_create_result = File::create(&tmp_filename);
        if let Ok(ref mut file) = file_create_result
            && let Some(content) = content
        {
            if let Err(e) = file.write_all(content.as_bytes()) {
                return Err(anyhow::anyhow!("Failed to write to temp file: {}", e));
            }
            return Ok(tmp_filename.to_string_lossy().into_owned());
        }
    }

    Err(anyhow::anyhow!(
        "Failed to create temp file after 3 attempts"
    ))
}
