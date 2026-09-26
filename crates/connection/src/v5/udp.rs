// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::net::SocketAddr;

use anyhow::{Context, Result};
use tokio::net::UdpSocket;

use shared::{log, system::trigger::Trigger};

use crypt::datagram::{DatagramCrypt, UdpToken};

use crate::consts::{LISTEN_ADDRESS, LISTEN_ADDRESS_V6};

// Bigger than the max wire datagram (1280 bytes), so recv never truncates
const RECV_BUFFER_SIZE: usize = 2048;

/// UDP leg of a tunnel session.
///
/// Relays datagrams between the local RDP client (mstsc, on the same
/// localhost port as the TCP forwarder) and the tunnel server, encrypting
/// with the session `DatagramCrypt` pair. Deliberately unreliable: no
/// reordering, no retransmission (RDPUDP handles reliability end to end).
///
/// Lifecycle is bound to the TCP tunnel: the relay stops when the passed
/// `stop` trigger fires or when the handle is dropped.
pub struct UdpRelay {
    stop: Trigger,
    local_addr: SocketAddr,
}

impl UdpRelay {
    /// Binds the local UDP socket on `local_port` and spawns the relay task.
    ///
    /// - `local_port`: local UDP listen port. The caller resolves the default:
    ///   the same port as the TCP forwarder listener (UDP and TCP port spaces
    ///   are independent), unless an explicit port was requested.
    /// - `tunnel_host` / `tunnel_port`: destination of the tunnel server. The
    ///   host is the same as the TCP tunnel's; the port may differ (the
    ///   server reports its resolved UDP relay port in the open response).
    /// - `inbound`: decrypts datagrams from the tunnel server (s2c key).
    /// - `outbound`: encrypts datagrams towards the tunnel server (c2s key).
    /// - `stop`: tunnel-level trigger; the relay dies with the TCP tunnel.
    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        local_port: u16,
        enable_ipv6: bool,
        tunnel_host: &str,
        tunnel_port: u16,
        token: UdpToken,
        mut inbound: DatagramCrypt,
        mut outbound: DatagramCrypt,
        stop: Trigger,
    ) -> Result<Self> {
        let listen_addr = format!(
            "{}:{}",
            if enable_ipv6 {
                LISTEN_ADDRESS_V6
            } else {
                LISTEN_ADDRESS
            },
            local_port
        );
        let local = UdpSocket::bind(&listen_addr)
            .await
            .context("Failed to create local UDP socket")?;
        let local_addr = local.local_addr()?;

        let remote_addr = tokio::net::lookup_host((tunnel_host, tunnel_port))
            .await
            .context("Failed to resolve tunnel server address")?
            .next()
            .context("Tunnel server address resolved to nothing")?;

        // The tunnel-facing socket must match the remote address family
        let tunnel = UdpSocket::bind(if remote_addr.is_ipv6() {
            "[::]:0"
        } else {
            "0.0.0.0:0"
        })
        .await
        .context("Failed to create tunnel UDP socket")?;
        tunnel
            .connect(remote_addr)
            .await
            .context("Failed to connect tunnel UDP socket")?;

        let relay = UdpRelay {
            stop: stop.clone(),
            local_addr,
        };

        tokio::spawn({
            let stop = stop.clone();
            async move {
                let mut local_buf = [0u8; RECV_BUFFER_SIZE];
                let mut tunnel_buf = [0u8; RECV_BUFFER_SIZE];
                // mstsc uses an ephemeral port; forward replies to the last
                // local address that sent us an authentic-looking datagram
                let mut last_local: Option<SocketAddr> = None;
                let mut discarded: u64 = 0;
                let mut invalid: u64 = 0;

                loop {
                    tokio::select! {
                        biased;
                        _ = stop.wait_async() => {
                            log::debug!("UDP relay stopping (tunnel closed)");
                            break;
                        }
                        res = local.recv_from(&mut local_buf) => {
                            match res {
                                Ok((len, addr)) => {
                                    last_local = Some(addr);
                                    match outbound.encrypt(&token, &local_buf[..len]) {
                                        Ok(datagram) => {
                                            if let Err(e) = tunnel.send(&datagram).await {
                                                log::error!("UDP relay: failed to send datagram to tunnel server: {e}");
                                            }
                                        }
                                        Err(e) => {
                                            // Oversized payload, etc: drop, do not kill the relay
                                            log::debug!("UDP relay: dropping local datagram: {e}");
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("UDP relay: local socket error: {e}");
                                    break;
                                }
                            }
                        }
                        res = tunnel.recv(&mut tunnel_buf) => {
                            match res {
                                Ok(len) => {
                                    match inbound.decrypt(&token, &tunnel_buf[..len]) {
                                        Ok(Some(payload)) => {
                                            if let Some(addr) = last_local
                                                && let Err(e) = local.send_to(&payload, addr).await
                                            {
                                                log::error!("UDP relay: failed to send datagram to local client: {e}");
                                            }
                                            // No local peer seen yet: drop silently
                                        }
                                        Ok(None) => {
                                            // Duplicate/out-of-window/not-for-us: benign on UDP
                                            discarded += 1;
                                        }
                                        Err(e) => {
                                            invalid += 1;
                                            log::debug!("UDP relay: discarding invalid tunnel datagram: {e}");
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("UDP relay: tunnel socket error: {e}");
                                    break;
                                }
                            }
                        }
                    }
                }
                log::debug!(
                    "UDP relay on {} stopped (discarded: {}, invalid: {})",
                    local_addr,
                    discarded,
                    invalid
                );
            }
        });

        Ok(relay)
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    pub fn stop(&self) {
        self.stop.trigger();
    }
}

impl Drop for UdpRelay {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypt::{
        datagram::{DatagramCrypt, TOKEN_LENGTH},
        types::SharedSecret,
    };

    // Mirror of the launcher crypt pair, acting as the tunnel server:
    // it encrypts with the launcher's inbound key and vice versa.
    fn server_crypts(key: &[u8; 32]) -> (DatagramCrypt, DatagramCrypt) {
        let key = SharedSecret::new(*key);
        (DatagramCrypt::new(&key), DatagramCrypt::new(&key))
    }

    #[tokio::test]
    async fn test_udp_relay_roundtrip() {
        log::setup_logging("debug", log::LogType::Test);

        let key = [9u8; 32];
        let token = [0x42u8; TOKEN_LENGTH];

        // Fake tunnel server: UDP socket that decrypts c2s datagrams and
        // answers with the payload uppercased, encrypted s2c
        let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_socket.local_addr().unwrap();
        let server_stop = Trigger::new();
        tokio::spawn({
            let server_stop = server_stop.clone();
            async move {
                let (mut server_in, mut server_out) = server_crypts(&key);
                let mut buf = [0u8; RECV_BUFFER_SIZE];
                loop {
                    tokio::select! {
                        _ = server_stop.wait_async() => break,
                        res = server_socket.recv_from(&mut buf) => {
                            let (len, peer) = res.unwrap();
                            let payload = server_in
                                .decrypt(&token, &buf[..len])
                                .unwrap()
                                .unwrap();
                            let reply: Vec<u8> =
                                payload.iter().map(u8::to_ascii_uppercase).collect();
                            let datagram = server_out.encrypt(&token, &reply).unwrap();
                            server_socket.send_to(&datagram, peer).await.unwrap();
                        }
                    }
                }
            }
        });

        // Launcher side relay: inbound/outbound with the same key (mirrored roles)
        let (launcher_out_key, launcher_in_key) = (key, key);
        let stop = Trigger::new();
        let relay = UdpRelay::start(
            0,
            false,
            &server_addr.ip().to_string(),
            server_addr.port(),
            token,
            DatagramCrypt::new(&SharedSecret::new(launcher_in_key)),
            DatagramCrypt::new(&SharedSecret::new(launcher_out_key)),
            stop.clone(),
        )
        .await
        .unwrap();

        // Local "mstsc" client on an ephemeral port
        let mstsc = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        mstsc
            .send_to(b"hello udp", relay.local_addr())
            .await
            .unwrap();

        let mut buf = [0u8; RECV_BUFFER_SIZE];
        let len = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            mstsc.recv(&mut buf),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(&buf[..len], b"HELLO UDP");

        // Stopping the relay frees the port
        let relay_addr = relay.local_addr();
        relay.stop();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        drop(relay);
        server_stop.trigger();

        // Port can be rebound once the relay is gone
        let rebound = UdpSocket::bind(relay_addr).await;
        assert!(rebound.is_ok());
    }

    #[tokio::test]
    async fn test_udp_relay_discards_garbage_and_survives() {
        log::setup_logging("debug", log::LogType::Test);

        let key = [3u8; 32];
        let token = [0x77u8; TOKEN_LENGTH];

        // Fake tunnel server: on the first valid datagram, answers first with
        // forged garbage (fails AEAD on the relay), then with a valid reply
        let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_socket.local_addr().unwrap();
        let server_stop = Trigger::new();
        tokio::spawn({
            let server_stop = server_stop.clone();
            async move {
                let (mut server_in, mut server_out) = server_crypts(&key);
                let mut buf = [0u8; RECV_BUFFER_SIZE];
                loop {
                    tokio::select! {
                        _ = server_stop.wait_async() => break,
                        res = server_socket.recv_from(&mut buf) => {
                            let (len, peer) = res.unwrap();
                            let payload = server_in
                                .decrypt(&token, &buf[..len])
                                .unwrap()
                                .unwrap();

                            // Garbage with a huge seq: must fail AEAD on the
                            // relay without poisoning its replay window
                            let mut forged = Vec::new();
                            forged.extend_from_slice(&token);
                            forged.extend_from_slice(&(u64::MAX - 1).to_be_bytes());
                            forged.extend_from_slice(&[0u8; 8]);
                            forged.extend_from_slice(&[0u8; 16]);
                            server_socket.send_to(&forged, peer).await.unwrap();

                            let reply: Vec<u8> =
                                payload.iter().map(u8::to_ascii_uppercase).collect();
                            let datagram = server_out.encrypt(&token, &reply).unwrap();
                            server_socket.send_to(&datagram, peer).await.unwrap();
                        }
                    }
                }
            }
        });

        let stop = Trigger::new();
        let relay = UdpRelay::start(
            0,
            false,
            &server_addr.ip().to_string(),
            server_addr.port(),
            token,
            DatagramCrypt::new(&SharedSecret::new(key)),
            DatagramCrypt::new(&SharedSecret::new(key)),
            stop.clone(),
        )
        .await
        .unwrap();

        // The relay must discard the forged datagram and still deliver the
        // valid reply that arrives right after it
        let mstsc = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        mstsc
            .send_to(b"still alive", relay.local_addr())
            .await
            .unwrap();

        let mut buf = [0u8; RECV_BUFFER_SIZE];
        let len = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            mstsc.recv(&mut buf),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(&buf[..len], b"STILL ALIVE");

        stop.trigger();
    }

    #[tokio::test]
    async fn test_udp_relay_explicit_local_and_remote_ports() {
        log::setup_logging("debug", log::LogType::Test);

        let key = [5u8; 32];
        let token = [0x11u8; TOKEN_LENGTH];

        // Fake tunnel server on a port DIFFERENT from the TCP tunnel port:
        // the relay must send datagrams to the port given, not any other
        let tcp_tunnel_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap(); // decoy
        let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_socket.local_addr().unwrap();
        assert_ne!(tcp_tunnel_socket.local_addr().unwrap().port(), server_addr.port());

        let server_stop = Trigger::new();
        tokio::spawn({
            let server_stop = server_stop.clone();
            async move {
                let (mut server_in, _server_out) = server_crypts(&key);
                let mut buf = [0u8; RECV_BUFFER_SIZE];
                loop {
                    tokio::select! {
                        _ = server_stop.wait_async() => break,
                        res = server_socket.recv_from(&mut buf) => {
                            let (len, _peer) = res.unwrap();
                            let payload = server_in
                                .decrypt(&token, &buf[..len])
                                .unwrap()
                                .unwrap();
                            assert_eq!(payload, b"ping");
                            server_stop.trigger();
                        }
                    }
                }
            }
        });

        // Explicit local UDP port: pick a free one, release it, then have the
        // relay bind it explicitly
        let free_port = UdpSocket::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap()
            .port();

        let stop = Trigger::new();
        let relay = UdpRelay::start(
            free_port,
            false,
            &server_addr.ip().to_string(),
            server_addr.port(),
            token,
            DatagramCrypt::new(&SharedSecret::new(key)),
            DatagramCrypt::new(&SharedSecret::new(key)),
            stop.clone(),
        )
        .await
        .unwrap();

        // The relay must be listening exactly on the requested port
        assert_eq!(relay.local_addr().port(), free_port);

        // Send a datagram; the fake server on the explicit remote port gets
        // it (and triggers server_stop on success)
        let mstsc = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        mstsc.send_to(b"ping", relay.local_addr()).await.unwrap();

        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            server_stop.wait_async(),
        )
        .await
        .unwrap();

        // Nothing must have arrived at the decoy (TCP port)
        let mut buf = [0u8; RECV_BUFFER_SIZE];
        let decoy = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            tcp_tunnel_socket.recv_from(&mut buf),
        )
        .await;
        assert!(decoy.is_err(), "decoy socket must not receive anything");

        stop.trigger();
    }
}
