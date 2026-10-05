// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use super::super::tests::helpers::*;
use super::*;

use std::time::Duration;

use tokio::{io::AsyncWriteExt, net::TcpListener};

use shared::log;

use super::super::{
    protocol::{PayloadWithChannel, payload_pair, payload_with_channel_pair},
    proxy::handler::ServerChannels,
};

#[tokio::test]
async fn test_stop_signal() -> Result<()> {
    log::setup_logging("debug", log::LogType::Test);

    let remote_server = dummy_remote_server().await;
    let stop = Trigger::new();
    let proxy = Proxy::new(
        &remote_server.listen_address(),
        dummy_ticket(),
        dummy_crypt_info(),
        &dummy_shared_secret(),
        Duration::from_secs(2),
        stop.clone(),
    );

    let (ctrl_tx, ctrl_rx) = Handler::new_command_channel();

    let stopped = Trigger::new();
    tokio::spawn({
        let stopped = stopped.clone();
        async move {
            if let Err(e) = proxy.run_task(ctrl_tx, ctrl_rx).await {
                log::error!("Proxy run_task error: {:?}", e);
            } else {
                stopped.trigger();
            }
        }
    });

    stop.trigger();
    stopped
        .wait_timeout_async(std::time::Duration::from_secs(1))
        .await
        .context("Proxy did not stop within timeout")?;
    Ok(())
}

#[tokio::test]
async fn test_proxy_connection_fail() {
    log::setup_logging("debug", log::LogType::Test);
    // Bind to port 0 to get a free port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    // Drop listener to close the port
    drop(listener);

    let proxy = Proxy::new(
        &addr.to_string(),
        dummy_ticket(),
        dummy_crypt_info(),
        &dummy_shared_secret(),
        Duration::from_secs(2),
        Trigger::new(),
    );

    // Should fail to connect
    let result = proxy.run().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_proxy_handshake_fail_garbage() {
    log::setup_logging("debug", log::LogType::Test);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // Spawn a dummy server that sends garbage
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let _ = socket.write_all(b"garbage data").await;
        }
    });

    let proxy = Proxy::new(
        &addr.to_string(),
        dummy_ticket(),
        dummy_crypt_info(),
        &dummy_shared_secret(),
        Duration::from_secs(2),
        Trigger::new(),
    );

    // Should fail during handshake
    let result = proxy.run().await;
    assert!(result.is_err());
    log::debug!(
        "Proxy handshake failed as expected: {:?}",
        result.err().unwrap().to_string()
    );
}

#[tokio::test]
async fn test_handler_request_channel() {
    let (ctrl_tx, ctrl_rx) = Handler::new_command_channel();
    let handler = Handler::new(ctrl_tx);

    let task = tokio::spawn(async move {
        if let Ok(cmd) = ctrl_rx.recv_async().await {
            match cmd {
                Command::RequestChannel {
                    channel_id,
                    response,
                } => {
                    assert_eq!(channel_id, 42);
                    // Create dummy channels to return
                    let (tx, _rx) = payload_with_channel_pair();
                    let (_tx2, rx) = payload_pair();

                    let channels = ServerChannels { tx, rx };
                    let _ = response.send_async(Ok(channels)).await;
                }
                _ => panic!("Unexpected command"),
            }
        }
    });

    let result = handler.request_channel(42).await;
    assert!(result.is_ok());
    task.await.unwrap();
}

#[tokio::test]
async fn test_handler_release_channel() {
    let (ctrl_tx, ctrl_rx) = Handler::new_command_channel();
    let handler = Handler::new(ctrl_tx);

    let task = tokio::spawn(async move {
        if let Ok(cmd) = ctrl_rx.recv_async().await {
            match cmd {
                Command::ReleaseChannel { channel_id } => {
                    assert_eq!(channel_id, 99);
                }
                _ => panic!("Unexpected command"),
            }
        }
    });

    let result = handler.release_channel(99).await;
    assert!(result.is_ok());
    task.await.unwrap();
}

#[tokio::test]
async fn test_connect() -> Result<()> {
    log::setup_logging("debug", log::LogType::Test);

    log::debug!("Creating proxy");
    let remote_server = dummy_remote_server().await;
    let stop = Trigger::new();
    let proxy = Proxy::new(
        &remote_server.listen_address(),
        dummy_ticket(),
        dummy_crypt_info(),
        &dummy_shared_secret(),
        Duration::from_secs(2),
        stop.clone(),
    );

    proxy.run().await.context("Failed to run proxy")?;
    // If result is ok, the connection is done, data has been sent and received

    stop.trigger();
    Ok(())
}

#[tokio::test]
async fn test_recv_data() -> Result<()> {
    log::setup_logging("debug", log::LogType::Test);

    log::debug!("Creating proxy");
    let remote_server = dummy_remote_server().await;
    let proxy = Proxy::new(
        &remote_server.listen_address(),
        dummy_ticket(),
        dummy_crypt_info(),
        &dummy_shared_secret(),
        Duration::from_secs(2),
        remote_server.stop.clone(),
    );

    let handler = proxy.run().await.context("Failed to run proxy")?;
    // Create a client
    let ServerChannels { tx: _tx, rx } = handler
        .request_channel(1)
        .await
        .context("Failed to request channel")?;

    // Send data to channel
    remote_server
        .tx
        .send_async(PayloadWithChannel::new(1, b"hello"))
        .await?;

    let data = rx
        .recv_async()
        .await
        .context("Failed to receive data from channel")?;

    log::debug!("Received data: {:?}", data);
    assert_eq!(data.as_ref(), b"hello");

    handler.release_channel(1).await?;

    remote_server.stop.trigger();
    Ok(())
}

#[tokio::test]
async fn test_send_data() -> Result<()> {
    log::setup_logging("debug", log::LogType::Test);

    log::debug!("Creating proxy");
    let remote_server = dummy_remote_server().await;
    let proxy = Proxy::new(
        &remote_server.listen_address(),
        dummy_ticket(),
        dummy_crypt_info(),
        &dummy_shared_secret(),
        Duration::from_secs(2),
        remote_server.stop.clone(),
    );

    let handler = proxy.run().await.context("Failed to run proxy")?;
    // Create a client
    let ServerChannels { tx, rx: _rx } = handler
        .request_channel(1)
        .await
        .context("Failed to request channel")?;

    log::debug!("Sending data to channel");
    // Send data to channel
    tx.send_async(PayloadWithChannel::new(1, b"hello"))
        .await
        .context("Failed to send data to channel")?;

    log::debug!("Waiting for data from remote server");
    let data = remote_server
        .rx
        .recv_async()
        .await
        .context("Failed to receive data from remote server")?;

    log::debug!("Received data: {:?}", data);
    assert_eq!(data.payload.as_ref(), b"hello");
    assert_eq!(data.channel_id, 1);

    handler.release_channel(1).await?;

    remote_server.stop.trigger();
    Ok(())
}

// ── Rekeying end-to-end: mock tunnel server with a persisted session ───────
//
// The mock keeps one pair of epoch-aware `Crypt`s for the whole session,
// across TCP legs, exactly like the real server keeps them on its `Session`:
// the ticket confirm and the `OpenResponse` of a `Recover` ride whatever epoch
// the persisted counters have already reached. The crypts are built from the
// launcher's own `TunnelRekeys` with the roles inverted (the mock writes on
// the launcher's inbound state and reads on its outbound one), so both sides
// derive the same per-epoch keys from the same PRK. A decrypt failure here is
// therefore a cryptographic proof that the two sides disagree on `k` or on the
// adoption seam — no key introspection needed.
mod rekey_mock {
    use std::sync::Arc;

    use tokio::{io::AsyncReadExt, net::TcpStream, sync::Mutex};

    use crypt::{
        datagram::TOKEN_LENGTH,
        secrets::build_tunnel_rekeys,
        tunnel::{Crypt, types::PacketBuffer},
    };

    use super::super::super::{
        protocol::consts::HANDSHAKE_V2_SIGNATURE,
        tests::helpers::{dummy_crypt_info, dummy_shared_secret, dummy_ticket},
    };
    use super::super::open_response::OpenResponse;
    use super::*;

    /// Session state the mock persists across legs. Only one leg is ever
    /// active (the proxy reconnects serially and a handler returns as soon as
    /// its leg is gone), and the lock is never held across an `.await` that can
    /// block on the socket: frame I/O runs on a private clone of the crypt,
    /// written back under the lock.
    pub struct MockState {
        /// Decrypts launcher→server frames (twin of the launcher's outbound).
        pub server_in: Crypt,
        /// Encrypts server→launcher frames (twin of the launcher's inbound).
        pub server_out: Crypt,
        /// Ticket the launcher must present next: the original on `Open`, the
        /// previously-issued session id on every `Recover`.
        pub pending: Ticket,
        pub conn_count: usize,
        /// Data frames echoed back, across all legs.
        pub echoes: usize,
        /// Set once a `Recover` handshake fully succeeded (the cross-epoch
        /// ticket confirm decrypted and the response was written).
        pub recover_seen: bool,
        /// Set when the second leg dies without ever carrying data: the
        /// launcher aborted after the announced-`k` check, before its outbound
        /// loop got the crypt.
        pub launcher_aborted: bool,
    }

    /// `k` is what the mock's crypts actually run on (always the honest session
    /// value); `announce_open` / `announce_recover` is what it writes into
    /// `OpenResponse.rekey_log2` (`announce_recover` may lie, to exercise the
    /// mismatch abort); `kill_after_leg1` drops the first leg once it has
    /// echoed that many data frames (0 = never).
    #[derive(Clone, Copy)]
    pub struct MockCfg {
        pub k: u8,
        pub announce_open: u8,
        pub announce_recover: u8,
        pub kill_after_leg1: usize,
    }

    pub struct RekeyMock {
        pub address: String,
        pub stop: Trigger,
        pub state: Arc<Mutex<MockState>>,
    }

    pub async fn start(cfg: MockCfg) -> RekeyMock {
        let stop = Trigger::new();
        let listener = crate::utils::create_listener(None, false).await.unwrap();
        let address = listener.local_addr().unwrap().to_string();

        // Same session material the test proxy is built from, so the PRK and
        // every derived epoch key matches the launcher's.
        let keys = dummy_crypt_info();
        let rekeys = build_tunnel_rekeys(&keys, &dummy_shared_secret(), &dummy_ticket(), cfg.k);
        let state = Arc::new(Mutex::new(MockState {
            server_in: Crypt::with_rekey(&keys.key_send, 0, rekeys.outbound),
            server_out: Crypt::with_rekey(&keys.key_receive, 0, rekeys.inbound),
            pending: dummy_ticket(),
            conn_count: 0,
            echoes: 0,
            recover_seen: false,
            launcher_aborted: false,
        }));

        tokio::spawn({
            let stop = stop.clone();
            let state = state.clone();
            async move {
                loop {
                    tokio::select! {
                        _ = stop.wait_async() => return,
                        accepted = listener.accept() => match accepted {
                            Ok((socket, _)) => {
                                let state = state.clone();
                                let handler_stop = stop.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_conn(state, socket, cfg, handler_stop).await {
                                        log::debug!("Rekey mock leg ended: {:?}", e);
                                    }
                                });
                            }
                            Err(e) => {
                                log::error!("Rekey mock accept error: {:?}", e);
                                stop.trigger();
                                return;
                            }
                        }
                    }
                }
            }
        });

        RekeyMock {
            address,
            stop,
            state,
        }
    }

    /// Reads one frame with the persisted inbound crypt, without holding the
    /// session lock while it waits for the peer.
    async fn read_frame(
        state: &Arc<Mutex<MockState>>,
        socket: &mut TcpStream,
        stop: &Trigger,
    ) -> Result<(Vec<u8>, u16)> {
        let mut crypt = state.lock().await.server_in.clone();
        let mut buf = PacketBuffer::new();
        let (data, channel) = crypt.read(stop, socket, &mut buf).await?;
        state.lock().await.server_in = crypt;
        Ok((data.to_vec(), channel))
    }

    /// Writes one frame with the persisted outbound crypt.
    async fn write_frame(
        state: &Arc<Mutex<MockState>>,
        socket: &mut TcpStream,
        stop: &Trigger,
        channel: u16,
        data: &[u8],
    ) -> Result<()> {
        let mut crypt = state.lock().await.server_out.clone();
        crypt.write(stop, socket, channel, data).await?;
        state.lock().await.server_out = crypt;
        Ok(())
    }

    async fn handle_conn(
        state: Arc<Mutex<MockState>>,
        mut socket: TcpStream,
        cfg: MockCfg,
        stop: Trigger,
    ) -> Result<()> {
        let conn = {
            let mut g = state.lock().await;
            g.conn_count += 1;
            g.conn_count
        };

        // Plaintext handshake: signature + cmd (1 = Open, 2 = Recover) + clear
        // ticket. A Recover also carries the declared seq pair, which this mock
        // ignores: it trusts its own persisted counters.
        let mut hdr = [0u8; 9];
        socket.read_exact(&mut hdr).await?;
        anyhow::ensure!(&hdr[..8] == HANDSHAKE_V2_SIGNATURE, "bad signature");
        let recover = match hdr[8] {
            1 => false,
            2 => true,
            c => anyhow::bail!("unexpected handshake command {c}"),
        };
        let mut clear = vec![0u8; if recover { 48 + 16 } else { 48 }];
        socket.read_exact(&mut clear).await?;
        let expected = state.lock().await.pending.as_ref().to_vec();
        anyhow::ensure!(
            clear[..48] == expected[..],
            "stale handshake ticket on connection {conn}"
        );

        // On a Recover the counters are already past zero and the ticket
        // confirm is itself a regular frame of the current epoch, so the seq
        // pair the response carries is the value BEFORE reading it — exactly
        // the `session.seqs()` snapshot the real server takes (the inbound
        // reader counter points at the NEXT expected seq, the outbound writer
        // one at the LAST seq used). The launcher recreates its crypts from
        // the same pair it reported on the failure, so both sides resume with
        // no gap and no replay, and `skip(inbound_seq - 1)` drains the whole
        // acknowledged window from the recovery buffer (a frame lost in flight
        // is the only thing that can still be replayed).
        let (inbound_seq, outbound_seq, announce) = {
            let g = state.lock().await;
            if recover {
                (
                    g.server_in.current_seq(),
                    g.server_out.current_seq(),
                    cfg.announce_recover,
                )
            } else {
                (1, 1, cfg.announce_open)
            }
        };

        let (confirm, channel) = read_frame(&state, &mut socket, &stop).await?;
        anyhow::ensure!(!confirm.is_empty(), "EOF waiting for handshake confirm");
        anyhow::ensure!(channel == 0, "confirm not on channel 0");
        anyhow::ensure!(
            confirm == expected,
            "ticket confirm mismatch on connection {conn}"
        );

        let new_id = Ticket::new([0x10u8 + conn as u8; 48]);
        let response = OpenResponse::new(
            new_id.clone(),
            1,
            inbound_seq,
            outbound_seq,
            [0u8; TOKEN_LENGTH],
            0,
            announce,
        )
        .as_vec();
        write_frame(&state, &mut socket, &stop, 0, &response).await?;
        {
            let mut g = state.lock().await;
            g.pending = new_id;
            if recover {
                g.recover_seen = true;
            }
        }

        loop {
            let (data, channel) = read_frame(&state, &mut socket, &stop).await?;
            if data.is_empty() {
                // EOF or stop. The launcher never closes a leg it accepted, so
                // a socket dying without ever carrying data on the second leg
                // means the proxy aborted inside `connect()`.
                if conn == 2 {
                    state.lock().await.launcher_aborted = true;
                }
                return Ok(());
            }
            if channel == 0 {
                // Control frames (OpenChannel / CloseChannel / Close / Nop)
                // consume the shared counter — that is the point — but their
                // content is irrelevant to this mock.
                continue;
            }
            write_frame(&state, &mut socket, &stop, channel, &data).await?;
            let mut g = state.lock().await;
            g.echoes += 1;
            let kill = cfg.kill_after_leg1 != 0 && conn == 1 && g.echoes >= cfg.kill_after_leg1;
            drop(g);
            if kill {
                log::debug!("Rekey mock: dropping leg 1 after echo");
                return Ok(());
            }
        }
    }

    pub async fn wait_for(
        state: &Arc<Mutex<MockState>>,
        pred: impl Fn(&MockState) -> bool,
        timeout: std::time::Duration,
    ) -> bool {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let guard = state.lock().await;
            if pred(&guard) {
                return true;
            }
            drop(guard);
            if tokio::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

use rekey_mock::{MockCfg, RekeyMock};

/// First leg with `k = 2`: the launcher adopts the threshold from the
/// `OpenResponse`, and the counters cross into epoch 1 mid-connection — the
/// handshake ticket is seq 1, so four data frames reach seqs 2..=5 and 5 is
/// already epoch 1 (`seq >> 2`), and the echoes advance the inbound counter the
/// same way. Every mock decrypt is a cross-check of the launcher's epoch-1 keys
/// against an independently derived PRK state: a wrong adoption seam fails the
/// AEAD tag, not an assertion.
#[tokio::test]
async fn test_proxy_adopts_rekey_and_survives_epoch_crossing() -> Result<()> {
    log::setup_logging("debug", log::LogType::Test);

    let mock: RekeyMock = rekey_mock::start(MockCfg {
        k: 2,
        announce_open: 2,
        announce_recover: 2,
        kill_after_leg1: 0,
    })
    .await;
    let stop = Trigger::new();
    let proxy = Proxy::new(
        &mock.address,
        dummy_ticket(),
        dummy_crypt_info(),
        &dummy_shared_secret(),
        Duration::from_secs(2),
        stop.clone(),
    );
    let udp_handle = proxy.udp_token_handle();

    let handler = proxy.run().await.context("Failed to run proxy")?;
    let ServerChannels { tx, rx } = handler.request_channel(1).await?;

    for i in 1..=4u8 {
        let payload = vec![b'k', i, b'!'];
        tx.send_async(PayloadWithChannel::new(1, &payload)).await?;
        let got = tokio::time::timeout(Duration::from_secs(5), rx.recv_async())
            .await
            .context("echo timed out")?
            .context("channel closed")?;
        assert_eq!(got.as_ref(), payload.as_slice(), "echo {i}");
    }

    let g = mock.state.lock().await;
    assert_eq!(g.conn_count, 1);
    assert_eq!(g.echoes, 4);
    // Outbound = ticket confirm (1) + OpenChannel (2) + the 4 data frames, so
    // the counter is firmly inside epoch 1 (`seq >> 2 >= 1` from seq 5 on) and
    // the mock authenticated every one of them.
    assert!(
        g.server_in.current_seq() >= 6,
        "outbound counter did not cross into epoch 1: {}",
        g.server_in.current_seq()
    );
    // Inbound = OpenResponse (1) + the 4 echoes.
    assert!(
        g.server_out.current_seq() >= 5,
        "inbound counter did not cross into epoch 1: {}",
        g.server_out.current_seq()
    );
    drop(g);

    // Adopted from the OpenResponse and pinned for the whole session; it also
    // rides the UDP handle so the relay builds its crypts with the same `k`.
    let pinned = udp_handle.lock().unwrap().map(|(_, _, k)| k);
    assert_eq!(pinned, Some(2));

    stop.trigger();
    mock.stop.trigger();
    Ok(())
}

/// The recover seam, which the first leg cannot cover: after `k = 2` data
/// pushed the persisted counters into epoch 1, the leg drops and the proxy
/// reconnects. The `Recover` ticket confirm goes out at a seq that is already
/// in epoch 1, and the mock can only decrypt it if the launcher installed the
/// pinned `k` on the crypts BEFORE writing the ticket — adopting it after the
/// `OpenResponse` (the first-leg seam) is too late here. The mock re-advertises
/// the same `k`, the recovery buffer is skipped from the seq the launcher
/// acknowledged, and traffic keeps flowing.
#[tokio::test]
async fn test_recover_uses_persisted_k_past_epoch_boundary() -> Result<()> {
    log::setup_logging("debug", log::LogType::Test);

    let mock = rekey_mock::start(MockCfg {
        k: 2,
        announce_open: 2,
        announce_recover: 2,
        kill_after_leg1: 2,
    })
    .await;
    let stop = Trigger::new();
    let proxy = Proxy::new(
        &mock.address,
        dummy_ticket(),
        dummy_crypt_info(),
        &dummy_shared_secret(),
        Duration::from_secs(2),
        stop.clone(),
    );

    let handler = proxy.run().await.context("Failed to run proxy")?;
    let ServerChannels { tx, rx } = handler.request_channel(1).await?;

    // Two echoes, then the mock drops the leg. Both frames sit in epoch 0, so
    // this part would pass even without rekeying; what it buys is a persisted
    // counter that has moved past the handshake.
    for i in 1..=2u8 {
        let payload = vec![b'r', i];
        tx.send_async(PayloadWithChannel::new(1, &payload)).await?;
        let got = tokio::time::timeout(Duration::from_secs(5), rx.recv_async())
            .await
            .context("echo timed out")?
            .context("channel closed")?;
        assert_eq!(got.as_ref(), payload.as_slice());
    }

    // The leg is gone; the proxy reconnects on its own. Wait on the mock's own
    // flag instead of guessing how long recovery takes: `recover_seen` is only
    // set after a cross-epoch ticket confirm decrypted under the PRK-derived
    // epoch-1 key.
    assert!(
        rekey_mock::wait_for(&mock.state, |g| g.recover_seen, Duration::from_secs(10)).await,
        "Recover handshake never completed: the cross-epoch ticket confirm did \
         not decrypt (was `k` not installed before the ticket frame?)"
    );

    // Queued in the proxy while the leg was down; only flows once the Recover
    // completes, riding a counter that is well into epoch 1.
    tx.send_async(PayloadWithChannel::new(1, b"post-recover"))
        .await?;
    let got = tokio::time::timeout(Duration::from_secs(10), rx.recv_async())
        .await
        .context("post-recover echo timed out (recovery failed?)")?
        .context("channel closed")?;
    assert_eq!(got.as_ref(), b"post-recover");

    let g = mock.state.lock().await;
    assert_eq!(g.conn_count, 2, "leg should have recovered exactly once");
    assert_eq!(g.echoes, 3);
    // Seqs on the persisted inbound counter: ticket (1), OpenChannel (2), two
    // data frames (3, 4), the cross-epoch recover confirm (5) and the
    // post-recover frame (6); `current_seq` then points at the next expected
    // seq, 7.
    assert!(
        g.server_in.current_seq() >= 7,
        "post-recover frame should ride epoch 1 or later: {}",
        g.server_in.current_seq()
    );
    drop(g);

    stop.trigger();
    mock.stop.trigger();
    Ok(())
}

/// A `Recover` that re-advertises a `k` different from the session's pinned
/// value means both sides would walk on incompatible keystreams while still
/// holding the AEAD-ticket confirm: a buggy or hostile far side. The launcher
/// must abort hard rather than silently corrupt the tunnel, and must not keep
/// reconnecting.
///
/// The mock's crypts still run on the honest `k = 2`, so the cross-epoch ticket
/// confirm decrypts and `recover_seen` gets set; the abort is triggered purely
/// by the announced value (3), before any epoch-3 frame has to exist.
#[tokio::test]
async fn test_recover_mismatched_k_aborts_proxy() -> Result<()> {
    log::setup_logging("debug", log::LogType::Test);

    let mock = rekey_mock::start(MockCfg {
        k: 2,
        announce_open: 2,
        announce_recover: 3, // lie
        kill_after_leg1: 1,
    })
    .await;
    let stop = Trigger::new();
    let proxy = Proxy::new(
        &mock.address,
        dummy_ticket(),
        dummy_crypt_info(),
        &dummy_shared_secret(),
        Duration::from_secs(2),
        stop.clone(),
    );
    let udp_handle = proxy.udp_token_handle();

    let handler = proxy.run().await.context("Failed to run proxy")?;
    let ServerChannels { tx, rx } = handler.request_channel(1).await?;

    tx.send_async(PayloadWithChannel::new(1, b"one")).await?;
    let got = tokio::time::timeout(Duration::from_secs(5), rx.recv_async())
        .await
        .context("echo timed out")?
        .context("channel closed")?;
    assert_eq!(got.as_ref(), b"one");

    // Leg 1 is gone; the Recover completes cryptographically (ticket confirm in
    // epoch 1 under the real k = 2), then the mismatch aborts the proxy.
    assert!(
        rekey_mock::wait_for(&mock.state, |g| g.recover_seen, Duration::from_secs(10)).await,
        "Recover handshake never completed (cross-epoch ticket failed?)"
    );
    let g = mock.state.lock().await;
    assert_eq!(g.conn_count, 2);
    assert!(
        g.server_in.current_seq() >= 4,
        "ticket confirm should have advanced the counter: {}",
        g.server_in.current_seq()
    );
    drop(g);

    // `connect()` returns the error, so the proxy task tears down: the socket
    // closes and the launcher never sends data on the second leg.
    assert!(
        rekey_mock::wait_for(&mock.state, |g| g.launcher_aborted, Duration::from_secs(10)).await,
        "proxy did not close the socket after the k mismatch"
    );
    assert!(
        !rekey_mock::wait_for(
            &mock.state,
            |g| g.conn_count > 2,
            Duration::from_millis(1500)
        )
        .await,
        "proxy kept reconnecting after the mismatch abort"
    );

    // The pinned session value was never polluted by the bogus announce: the
    // abort happens before `udp_token` is written, so it still carries the
    // honest `k` from the Open.
    let pinned = udp_handle.lock().unwrap().map(|(_, _, k)| k);
    assert_eq!(
        pinned,
        Some(2),
        "a mismatched announce must not overwrite the pinned session value"
    );

    // And nothing further reaches the server side of the tunnel: tearing the
    // proxy down either closes the channel (recv errors) or simply never
    // delivers anything again (recv stalls).
    match tokio::time::timeout(Duration::from_secs(5), rx.recv_async()).await {
        Err(_elapsed) => {}
        Ok(Err(_closed)) => {}
        Ok(Ok(p)) => panic!("data should not flow after the abort: {p:?}"),
    }

    stop.trigger();
    mock.stop.trigger();
    Ok(())
}
