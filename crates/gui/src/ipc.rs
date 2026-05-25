use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{
    GenericNamespaced, Listener, ListenerNonblockingMode, ListenerOptions, Stream,
};
use serde::{Deserialize, Serialize};

use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

fn socket_name(server_id: &str) -> io::Result<interprocess::local_socket::Name<'static>> {
    // GenericNamespaced → abstract namespace on Linux, named pipe on Windows,
    //                       /tmp/<name> on macOS (via SpecialDirUdSocket).
    server_id
        .to_string()
        .to_ns_name::<GenericNamespaced>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))
}

#[derive(Serialize, Deserialize, Clone, Zeroize, ZeroizeOnDrop)]
pub struct RailLaunchMsg {
    pub app: String,
    pub args: String,
    pub working_dir: String,
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
    let name = socket_name(server_id)?;
    // try_overwrite(true) unlinks any stale socket file before binding,
    // so a previous crash won't leave a dead file behind (relevant on macOS/Unix).
    let opts = ListenerOptions::new()
        .name(name)
        .try_overwrite(true)
        .nonblocking(ListenerNonblockingMode::Accept);
    let listener = opts.create_sync().map_err(io::Error::other)?;
    let listener = Arc::new(Mutex::new(Some(listener)));

    let listener_clone = listener.clone();
    let handle = std::thread::spawn({
        let expected_token = Zeroizing::new(expected_token.to_string());
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
                        let _ =
                            serde_json::from_reader::<_, RailLaunchMsg>(stream).map(|mut msg| {
                                // Token verification
                                if msg.server_token != *expected_token {
                                    shared::log::warn!(
                                        "IPC: rejected RAIL app launch — token mismatch for {}",
                                        msg.app,
                                    );
                                    msg.zeroize();
                                    return;
                                }
                                shared::log::info!("IPC: received RAIL app launch: {}", msg.app,);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_launch_msg_round_trip() {
        let msg = RailLaunchMsg {
            app: "notepad.exe".into(),
            args: "file.txt".into(),
            working_dir: "C:\\temp".into(),
            server_token: "secret123".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: RailLaunchMsg = serde_json::from_str(&json).unwrap();
        assert_eq!(back.app, msg.app);
        assert_eq!(back.args, msg.args);
        assert_eq!(back.working_dir, msg.working_dir);
        assert_eq!(back.server_token, msg.server_token);
    }

    #[test]
    fn rail_launch_msg_json_fields() {
        let msg = RailLaunchMsg {
            app: "app".into(),
            args: "".into(),
            working_dir: "".into(),
            server_token: "tok".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"app\""));
        assert!(json.contains("\"args\""));
        assert!(json.contains("\"working_dir\""));
        assert!(json.contains("\"server_token\""));
        assert!(json.contains("\"app\""));
        assert!(json.contains("\"tok\""));
    }
}
