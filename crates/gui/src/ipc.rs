use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, Listener, ListenerNonblockingMode, ListenerOptions, Stream,
};
use serde::{Deserialize, Serialize};

fn socket_name(server_id: &str) -> io::Result<interprocess::local_socket::Name<'static>> {
    // GenericNamespaced: abstract namespace on Linux, named pipe on Windows
    if let Ok(name) = server_id.to_string().to_ns_name::<GenericNamespaced>() {
        return Ok(name);
    }
    // Fallback for macOS: absolute filesystem path in /tmp
    format!("/tmp/uds_rail_{server_id}")
        .to_fs_name::<GenericFilePath>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RailLaunchMsg {
    pub rail_app: String,
    pub rail_args: String,
    pub rail_working_dir: String,
    pub server_token: String,
}

/// Handle to a running IPC listener. Drop it to stop listening.
pub struct IpcListener {
    listener: Arc<Mutex<Option<Listener>>>,
    _thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for IpcListener {
    fn drop(&mut self) {
        *self.listener.lock().unwrap() = None;
    }
}

/// Try to send a RAIL launch request via IPC to an already-running session.
/// Returns `true` if the message was delivered (someone was listening).
pub fn try_send(server_id: &str, msg: &RailLaunchMsg) -> bool {
    let Ok(name) = socket_name(server_id) else {
        return false;
    };
    match Stream::connect(name) {
        Ok(mut stream) => serde_json::to_writer(&mut stream, msg).is_ok(),
        Err(_) => false,
    }
}

/// Bind an IPC listener and spawn a background thread that calls `on_launch` for
/// each received message that passes token verification.
/// If `expected_token` is set, messages without a matching `server_token` are silently dropped.
pub fn bind(
    server_id: &str,
    expected_token: &str,
    on_launch: impl Fn(RailLaunchMsg) + Send + 'static,
) -> io::Result<IpcListener> {
    // Clean up stale socket file from a previous crash (macOS only — no-op elsewhere)
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = std::fs::remove_file(format!("/tmp/uds_rail_{server_id}"));
    }
    let name = socket_name(server_id)?;
    let opts = ListenerOptions::new()
        .name(name)
        .nonblocking(ListenerNonblockingMode::Accept);
    let listener = opts.create_sync().map_err(io::Error::other)?;
    let listener = Arc::new(Mutex::new(Some(listener)));

    let listener_clone = listener.clone();
    let handle = std::thread::spawn({
        let expected_token = expected_token.to_string();
        move || {
            loop {
                let maybe_stream = {
                    let guard = listener_clone.lock().unwrap();
                    match guard.as_ref() {
                        Some(listener) => listener.accept(),
                        None => break,
                    }
                };
                match maybe_stream {
                    Ok(stream) => {
                        let _ = serde_json::from_reader::<_, RailLaunchMsg>(stream).map(|msg| {
                            // Token verification
                            if msg.server_token != expected_token {
                                shared::log::warn!(
                                    "IPC: rejected RAIL app launch — token mismatch for {}",
                                    msg.rail_app,
                                );
                                return;
                            }
                            shared::log::info!("IPC: received RAIL app launch: {}", msg.rail_app,);
                            on_launch(msg);
                        });
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => break,
                }
            }
        }
    });

    Ok(IpcListener {
        listener,
        _thread: Some(handle),
    })
}
