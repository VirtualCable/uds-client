// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! URP (UDS Remote Protocol) session window: drives a
//! `uds_rp::client::Session` from the launcher's winit loop, rendering the
//! shared display framebuffer with the two-pass XOR cursor (protocol §10)
//! and forwarding input as Set-1 scancodes + normalized pointer moves.
//!
//! Scaling policy: the remote resolution is fixed at connect time (the
//! launcher never renegotiates it; the protocol's DesktopResize stays a
//! lab-client feature for now) and the remote image is always stretched
//! to the window size by wgpu.

use std::net::ToSocketAddrs;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use anyhow::Result;
use flume::Receiver;
use rdp::integrations::ClipboardIntegration;
use shared::log;
use tokio::sync::oneshot;
use uds_rp::client::{
    ClientCommand, ClientEvent, CommandSender, Displays, EVENT_CHANNEL_CAPACITY, Session,
    SessionCredentials,
};
use uds_rp::core::InputMessage;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;

use crate::altgr::{AltGr, CtrlAction};
use crate::types::{UrpSettings, UrpWindowMode};
use crate::draw::ui::waves::{self, Wave};
use crate::wgpu_render::{CursorSprite, OverlayParams, WgpuRenderer};
use crate::windows::dialog::{
    DialogButton, DialogField, DialogFieldType, DialogResult, DialogSpec, DialogState,
};
use crate::{AppHandler, RawKey, WindowKind};

/// Fallback remote resolution when the broker does not fix one and the
/// monitor geometry cannot be determined.
const DEFAULT_RESOLUTION: (u32, u32) = (1920, 1080);

/// Local clipboard watcher callback: announces local clipboard text to the
/// session, swallowing the echo of a text we just applied from the remote.
struct UrpClipboardCallback {
    commands: CommandSender,
    suppress: Arc<Mutex<Option<String>>>,
}

impl rdp::integrations::ClipboardCallback for UrpClipboardCallback {
    fn send_text_is_available(&self, text: &str) {
        let mut sup = self.suppress.lock().unwrap();
        if sup.as_deref() == Some(text) {
            *sup = None;
            return;
        }
        drop(sup);
        let _ = self
            .commands
            .try_send(ClientCommand::ClipboardChanged(text.to_string()));
    }
}

pub struct UrpState {
    pub window: Arc<winit::window::Window>,
    pub renderer: WgpuRenderer,
    pub events_rx: Receiver<ClientEvent>,
    pub commands: CommandSender,
    pub displays: Displays,
    pub result_rx: Receiver<std::result::Result<(), String>>,
    pub keys_rx: Receiver<RawKey>,
    /// Remote desktop geometry announced by the server (display 0).
    pub desktop_size: (u32, u32),
    /// False until the server announces the primary display; until then the
    /// window shows a connecting message instead of an empty canvas.
    pub desktop_ready: bool,
    /// Phase of the connecting animation (waves move while it is not ready).
    pub connecting_anim: f32,
    pub connecting_waves: Vec<Wave>,
    pub connecting_logo: crate::logo::LogoImage,
    /// Scratch buffer for packed dirty-rect uploads.
    pub scratch: Vec<u8>,
    /// Last known local pointer position (physical window pixels).
    pub pointer: (f64, f64),
    /// Current remote cursor sprite; None until the first shape arrives.
    pub cursor: Option<CursorSprite>,
    /// Whether the remote cursor is visible (server-driven, §10).
    pub cursor_visible: bool,
    pub fullscreen: bool,
    /// True while the remote console is on a secure desktop (§18.41).
    pub secure_desktop: bool,
    /// Storage channel enabled by the transport policy (Alt+U / Alt+D).
    pub storage_enabled: bool,
    /// Spool entries from the last `StorageList`, kept while the picker
    /// dialog is open to map the chosen label back to the file name.
    pub spool_entries: Vec<(String, u64)>,
    /// Set by the `StorageList` event; drained by `process_urp_updates`
    /// (which owns the `ActiveEventLoop` needed to build the dialog).
    pub spool_dialog_pending: bool,
    /// Reply channel of the open spool picker dialog, if any.
    pub spool_reply: Option<oneshot::Receiver<DialogResult>>,
    altgr: AltGr,
    lctrl_held: bool,
    clipboard: Option<channels::clipboard::ClipboardHandle>,
    clipboard_suppress: Arc<Mutex<Option<String>>>,
}

/// Result of handling one session event.
pub enum UrpAction {
    /// Redraw the window.
    Continue,
    /// End the session and exit the launcher.
    Disconnect,
}

/// Surface-pixel rect the desktop image occupies: always the full window
/// (the remote image is stretched to fit; the remote resolution is fixed
/// at connect time).
#[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
pub fn dest_rect(win: (u32, u32)) -> (f32, f32, f32, f32) {
    (0.0, 0.0, win.0 as f32, win.1 as f32)
}

/// Maps a window position to protocol normalized pointer coordinates
/// (0..=65535 over the desktop, §18.37), honoring the letterbox rect.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped into u16 range"
)]
pub fn normalize_pointer(pos: (f64, f64), dest: (f32, f32, f32, f32)) -> (u16, u16) {
    let (dx, dy, dw, dh) = dest;
    if dw <= 0.0 || dh <= 0.0 {
        return (0, 0);
    }
    let nx = ((pos.0 as f32 - dx) / dw * 65535.0)
        .round()
        .clamp(0.0, 65535.0);
    let ny = ((pos.1 as f32 - dy) / dh * 65535.0)
        .round()
        .clamp(0.0, 65535.0);
    (nx as u16, ny as u16)
}

/// winit key → Set-1 scancode + extended flag, reusing the RDP table
/// (bit 0x0100 marks extended scancodes there).
fn urp_scancode(key: &KeyCode) -> Option<(u16, bool)> {
    let v = crate::keymap::RdpScanCode::get_from_key(Some(key))? as u16;
    Some((v & 0x00FF, v & 0x0100 != 0))
}

/// Wave layout of the connecting screen (same look as the progress window).
fn connecting_waves() -> Vec<Wave> {
    vec![
        Wave { y_base: 0.4, amplitude: 25.0, speed: 0.04, offset: 0.0, thickness: 5.0, opacity: 0.5 },
        Wave { y_base: 0.42, amplitude: 20.0, speed: 0.06, offset: 2.0, thickness: 3.5, opacity: 0.3 },
        Wave { y_base: 0.38, amplitude: 22.0, speed: 0.03, offset: 4.0, thickness: 6.0, opacity: 0.25 },
    ]
}

fn handle_urp_event(state: &mut UrpState, event: ClientEvent) -> UrpAction {
    log::trace!("URP event: {event:?}");
    match event {
        ClientEvent::Connected { udp } => {
            log::info!("URP session connected (udp={udp})");
            UrpAction::Continue
        }
        ClientEvent::DisplayInfo {
            display_id,
            width,
            height,
            codec,
        } => {
            log::info!("URP display {display_id}: {width}x{height} codec={codec:?}");
            if display_id == 0 {
                state.desktop_size = (width, height);
                state.desktop_ready = true;
                state.window.set_title("UDS Remote Desktop");
            }
            UrpAction::Continue
        }
        ClientEvent::FrameDirty { .. } => UrpAction::Continue,
        ClientEvent::CursorVisibility { visible } => {
            state.cursor_visible = visible;
            UrpAction::Continue
        }
        ClientEvent::CursorShape {
            width,
            height,
            hot_x,
            hot_y,
            rgba,
            xor_mask,
        } => {
            state.cursor = Some(
                state
                    .renderer
                    .make_cursor_sprite(width, height, hot_x, hot_y, &rgba, &xor_mask),
            );
            UrpAction::Continue
        }
        ClientEvent::ClipboardData { text } => {
            if let Some(ref cb) = state.clipboard {
                *state.clipboard_suppress.lock().unwrap() = Some(text.clone());
                if let Err(e) = cb.set_text(&text) {
                    log::warn!("URP clipboard set_text failed: {e}");
                }
            }
            UrpAction::Continue
        }
        ClientEvent::ConsoleConflict { user, state: cs } => {
            // Default consumer answer (§18.53): the ticket IS the
            // authorization, so we take the console over.
            log::warn!("URP console conflict (user={user:?}, state={cs}); taking over");
            let _ = state
                .commands
                .try_send(ClientCommand::AnswerConflict { take_over: true });
            UrpAction::Continue
        }
        ClientEvent::SecureDesktop { active } => {
            state.secure_desktop = active;
            UrpAction::Continue
        }
        ClientEvent::Status(s) => {
            log::info!("URP status: {s}");
            UrpAction::Continue
        }
        ClientEvent::Error(e) => {
            log::warn!("URP session warning: {e}");
            UrpAction::Continue
        }
        ClientEvent::Disconnected => UrpAction::Disconnect,
        ClientEvent::StorageList { entries } => {
            log::debug!("URP spool list: {} entries", entries.len());
            state.spool_entries = entries;
            state.spool_dialog_pending = true;
            UrpAction::Continue
        }
        ClientEvent::TransferDone {
            direction,
            name,
            bytes,
            ..
        } => {
            log::info!("URP {direction:?} finished: {name} ({bytes} bytes)");
            UrpAction::Continue
        }
        ClientEvent::TransferError { message, .. } => {
            log::warn!("URP transfer error: {message}");
            UrpAction::Continue
        }
        ClientEvent::TransferProgress { .. } => UrpAction::Continue,
        ClientEvent::PrintJobDone { path, bytes } => {
            log::info!("URP print job done: {} ({} bytes)", path.display(), bytes);
            if let Err(e) = open::that_detached(&path) {
                log::warn!("URP could not open printed PDF {}: {e}", path.display());
            }
            UrpAction::Continue
        }
        _ => UrpAction::Continue,
    }
}

impl UrpState {
    /// Drains the local key queue, translating to protocol input with the
    /// AltGr↔LCtrl pairing (Windows sends a fake LCtrl around AltGr).
    fn drain_keys(&mut self) {
        while let Ok(raw) = self.keys_rx.try_recv() {
            let Some((scancode, extended)) = urp_scancode(&raw.keycode) else {
                continue;
            };
            let now = Instant::now();
            match raw.keycode {
                KeyCode::ControlLeft => {
                    if raw.pressed {
                        self.altgr.ctrl_down(now);
                        self.lctrl_held = true;
                    } else {
                        self.lctrl_held = false;
                        self.altgr.ctrl_up();
                    }
                }
                KeyCode::AltRight if raw.pressed => {
                    if matches!(
                        self.altgr.altgr_down(now, self.lctrl_held),
                        CtrlAction::Press
                    ) {
                        let _ = self
                            .commands
                            .try_send(ClientCommand::Input(InputMessage::Key {
                                down: true,
                                scancode: 0x1D, // LControl
                                extended: false,
                            }));
                    }
                }
                _ => {}
            }
            let _ = self
                .commands
                .try_send(ClientCommand::Input(InputMessage::Key {
                    down: raw.pressed,
                    scancode,
                    extended,
                }));
            if raw.keycode == KeyCode::AltRight
                && !raw.pressed
                && matches!(self.altgr.altgr_up(), CtrlAction::Release)
            {
                let _ = self
                    .commands
                    .try_send(ClientCommand::Input(InputMessage::Key {
                        down: false,
                        scancode: 0x1D, // LControl
                        extended: false,
                    }));
            }
        }
    }

    /// Focus lost: forget the AltGr pairing state (pending key-ups may
    /// never arrive while unfocused).
    pub(crate) fn reset_altgr(&mut self) {
        self.altgr.reset();
        self.lctrl_held = false;
    }

    /// Opens a native file picker and uploads the chosen file to the
    /// server spool. The dialog runs on its own thread so the winit loop
    /// never blocks on it.
    pub(crate) fn start_upload(&self) {
        if !self.storage_enabled {
            return;
        }
        let commands = self.commands.clone();
        let spawned = std::thread::Builder::new()
            .name("uds-rp-upload-pick".to_string())
            .spawn(move || {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    let _ = commands.try_send(ClientCommand::StorageUpload { path });
                }
            });
        if let Err(e) = spawned {
            log::warn!("URP upload picker thread failed: {e}");
        }
    }

    /// Requests the server spool listing; the answer arrives as a
    /// `ClientEvent::StorageList` and opens the download picker dialog.
    pub(crate) fn request_spool_list(&self) {
        if self.storage_enabled {
            let _ = self.commands.try_send(ClientCommand::StorageList);
        }
    }

    /// Sends the current pointer position to the server.
    pub(crate) fn send_pointer_move(&self) {
        let size = self.window.inner_size();
        let dest = dest_rect((size.width, size.height));
        let (x, y) = normalize_pointer(self.pointer, dest);
        let _ = self
            .commands
            .try_send(ClientCommand::Input(InputMessage::PointerMove {
                display_id: 0,
                x,
                y,
            }));
    }

    /// Draws the connecting screen (animated waves and the UDS logo) while the
    /// server has not announced a display yet.
    fn render_connecting(&mut self) {
        let size = self.window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        let Some(mut pixmap) = tiny_skia::Pixmap::new(w, h) else {
            return;
        };
        pixmap.fill(tiny_skia::Color::from_rgba8(30, 30, 35, 255));
        let wave_data = waves::render(w, h, self.connecting_anim, 1.0, &self.connecting_waves);
        if let Some(size) = tiny_skia::IntSize::from_wh(w, h)
            && let Some(wave_pix) = tiny_skia::Pixmap::from_vec(wave_data, size)
        {
            pixmap.draw_pixmap(
                0,
                0,
                wave_pix.as_ref(),
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::identity(),
                None,
            );
        }
        let mut bgra = pixmap.data().to_vec();
        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        self.renderer
            .upload_gdi(&bgra, w, h, Some(&[(0, 0, w, h)]));

        let logo = &self.connecting_logo;
        let logo_x = (w as f32 - logo.width as f32) / 2.0;
        let logo_y = (h as f32 * 0.35 - logo.height as f32 / 2.0).max(0.0);
        let overlays = [OverlayParams {
            rgba: &logo.rgba,
            width: logo.width,
            height: logo.height,
            x: logo_x,
            y: logo_y,
            scale: 1.0,
        }];
        let sections = vec![
            crate::wgpu_render::OwnedSection::default()
                .with_layout(
                    wgpu_text::glyph_brush::Layout::default()
                        .h_align(wgpu_text::glyph_brush::HorizontalAlign::Center),
                )
                .with_screen_position((w as f32 / 2.0, logo_y + logo.height as f32 + 24.0))
                .add_text(
                    wgpu_text::glyph_brush::OwnedText::new(
                        "Connecting to the virtual machine...",
                    )
                    .with_scale(22.0)
                    .with_color([0.85, 0.85, 0.9, 1.0]),
                ),
        ];
        self.renderer
            .render_urp(&[], w, h, &overlays, &sections, None, None, None);
    }

    /// Renders one frame: uploads pending damage from the shared framebuffer
    /// and presents it with the software cursor on top.
    pub fn update_screen(&mut self) {
        if !self.desktop_ready {
            self.render_connecting();
            return;
        }
        let Some(display) = self.displays.get(0) else {
            return;
        };
        let Ok(display) = display.read() else {
            return;
        };
        let (w, h) = (display.width(), display.height());
        if w == 0 || h == 0 {
            return;
        }

        // Today's damage granularity is the full frame (§8/§15.1), but honor
        // the rect list: copy each rect packed and upload in one call.
        let rect = display.full_rect();
        self.scratch.clear();
        display.copy_rect_to(rect, &mut self.scratch);
        if self.scratch.is_empty() {
            return;
        }
        #[cfg(target_os = "macos")]
        {
            // The gdi texture is Rgba8 on macOS (Bgra8 elsewhere); the
            // framebuffer is always BGRA.
            for px in self.scratch.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
        }
        self.renderer
            .upload_gdi(&self.scratch, w, h, Some(&[(0, 0, w, h)]));

        let size = self.window.inner_size();
        let dest = dest_rect((size.width, size.height));
        let cursor = if self.cursor_visible {
            self.cursor
                .as_ref()
                .and_then(|sprite| sprite.draw_at(self.pointer, dest, (w, h)))
        } else {
            None
        };

        let mut sections = Vec::new();
        if self.secure_desktop {
            sections.push(
                crate::wgpu_render::OwnedSection::default()
                    .with_screen_position((16.0, 16.0))
                    .add_text(
                        wgpu_text::glyph_brush::OwnedText::new(
                            "Secure desktop (UAC / lock screen) — low rate, sanitized input",
                        )
                        .with_scale(16.0)
                        .with_color([1.0, 0.85, 0.55, 1.0]),
                    ),
            );
        }

        self.renderer
            .render_urp(&[], w, h, &[], &sections, cursor.as_ref(), None, Some(dest));
    }
}

impl AppHandler {
    pub(crate) fn open_urp(&mut self, el: &ActiveEventLoop, settings: UrpSettings) -> Result<()> {
        self.close_progress(); // Ensure progress is closed if it was open before
        log::info!("open_urp: {settings:?}");

        let monitor_scale = crate::monitor::scale(0);
        let (mon_w, mon_h) = crate::monitor::size(0).unwrap_or(DEFAULT_RESOLUTION);
        let (mon_lw, mon_lh) =
            crate::monitor::phys_2_logic((mon_w as i32, mon_h as i32), monitor_scale);
        let fixed = settings.screen_width > 0 && settings.screen_height > 0;
        let (remote_w, remote_h) = if fixed {
            (settings.screen_width, settings.screen_height)
        } else {
            (mon_lw.max(1) as u32, mon_lh.max(1) as u32)
        };
        let pref_resolution = (
            u16::try_from(remote_w).unwrap_or(u16::MAX),
            u16::try_from(remote_h).unwrap_or(u16::MAX),
        );

        let window = Arc::new(
            el.create_window(
                winit::window::Window::default_attributes()
                    .with_visible(false)
                    .with_title("UDS Remote Desktop - connecting...")
                    .with_inner_size(winit::dpi::LogicalSize::new(
                        remote_w as f64,
                        remote_h as f64,
                    ))
                    .with_window_icon(Some(crate::logo::load_icon())),
            )?,
        );
        let wid = window.id();
        let window_mode = settings.window_mode;
        match window_mode {
            UrpWindowMode::Fullscreen => {
                window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            }
            UrpWindowMode::Maximized => window.set_maximized(true),
            UrpWindowMode::Windowed | UrpWindowMode::Minimized => {}
        }
        let phys = window.inner_size();
        let renderer = WgpuRenderer::new(window.clone(), phys.width, phys.height)?;

        let addr = (settings.server.as_str(), settings.port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| {
                anyhow::anyhow!("could not resolve {}:{}", settings.server, settings.port)
            })?;

        let credentials = match (settings.user, settings.password) {
            (Some(user), Some(password)) => Some(SessionCredentials {
                user,
                domain: settings.domain.unwrap_or_default(),
                password,
            }),
            _ => None,
        };

        // Device integrations (§15.3): the uds-rp `channels` crate owns the
        // OS device stacks (cpal audio, nokhwa webcam, PC/SC smartcard) — the
        // same backends the labclient validated; each `Some` offers its
        // channel in the HELLO. Probing failures degrade to `None` (channel
        // simply not offered), they never abort the connection.
        let red = settings.redirections;
        let integrations = uds_rp::client::integrations::Integrations {
            webcam: red
                .webcam
                .then(uds_rp_channels::webcam::default_source)
                .flatten(),
            printer: if red.printing {
                // Phase 0: PDF-to-directory sink; PrintJobDone opens the PDF
                // with the system viewer.
                let dir = std::env::temp_dir().join(format!("uds-rp-print-{}", std::process::id()));
                match uds_rp_channels::printer::PdfToDirSink::new(dir, uds_rp::core::MAX_JOB_BYTES)
                {
                    Ok(sink) => Some(Box::new(sink) as _),
                    Err(e) => {
                        log::warn!("URP printer redirection unavailable: {e}");
                        None
                    }
                }
            } else {
                None
            },
            mic: red
                .mic
                .then(uds_rp_channels::audio::input::default_mic_factory),
            audio_out: red
                .audio
                .then(uds_rp_channels::audio::output::default_output_factory),
            smartcard: red
                .smartcard
                .then(uds_rp_channels::smartcard::default_backend)
                .flatten(),
        };

        let config = uds_rp::client::ClientConfig {
            addr,
            ticket: settings.ticket,
            key: settings.key,
            audio: settings.redirections.audio,
            audio_dump: None,
            mic: if settings.redirections.mic {
                uds_rp::client::mic::MicMode::Device
            } else {
                uds_rp::client::mic::MicMode::Off
            },
            // Privacy screen stays off: the server ignores the flag anyway
            // (incompatible with console-mirror capture; the IddCx work is
            // archived in the uds-rp `archive/iddcx-display` branch).
            privacy_screen: false,
            integrations,
            pref_resolution,
            credentials,
        };

        let (events_tx, events_rx) = flume::bounded(EVENT_CHANNEL_CAPACITY);
        let (session, commands, displays) = Session::new(config, events_tx);
        let (result_tx, result_rx) = flume::bounded(1);
        std::thread::Builder::new()
            .name("uds-rp-session".to_string())
            .spawn(move || {
                connection::tasks::mark_remote_session_running();
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime");
                let result = runtime.block_on(session.run());
                connection::tasks::mark_remote_session_not_running();
                if let Err(ref e) = result {
                    log::error!("URP session ended with error: {e:#}");
                }
                let _ = result_tx.send(result.map_err(|e| format!("{e:#}")));
            })?;

        let (clipboard, clipboard_suppress) = if settings.redirections.clipboard {
            let handle = channels::clipboard::ClipboardHandle::new();
            let suppress: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            handle.start(Arc::new(UrpClipboardCallback {
                commands: commands.clone(),
                suppress: suppress.clone(),
            }));
            (Some(handle), suppress)
        } else {
            (None, Arc::new(Mutex::new(None)))
        };

        let state = UrpState {
            window,
            renderer,
            events_rx,
            commands,
            displays,
            result_rx,
            keys_rx: self.keys_rx.clone(),
            desktop_size: (remote_w, remote_h),
            desktop_ready: false,
            connecting_anim: 0.0,
            connecting_waves: connecting_waves(),
            connecting_logo: crate::logo::load_logo(),
            scratch: Vec::new(),
            pointer: (0.0, 0.0),
            cursor: None,
            cursor_visible: true,
            fullscreen: window_mode == UrpWindowMode::Fullscreen,
            secure_desktop: false,
            storage_enabled: settings.redirections.storage,
            spool_entries: Vec::new(),
            spool_dialog_pending: false,
            spool_reply: None,
            altgr: AltGr::default(),
            lctrl_held: false,
            clipboard,
            clipboard_suppress,
        };
        self.urp = Some(Box::new(state));
        self.register_window(wid, WindowKind::Urp);
        while self.keys_rx.try_recv().is_ok() {}
        self.processing_events.store(true, Ordering::Relaxed);
        if let Some(ref state) = self.urp {
            state.window.set_cursor_visible(false);
            state.window.set_visible(true);
            if window_mode == UrpWindowMode::Minimized {
                state.window.set_minimized(true);
            }
            state.window.request_redraw();
        }
        Ok(())
    }

    pub(crate) fn close_urp(&mut self) {
        self.processing_events.store(false, Ordering::Relaxed);
        if let Some(state) = self.urp.take() {
            if let Some(ref cb) = state.clipboard {
                cb.stop();
            }
            self.unregister_window(state.window.id());
            // Dropping `commands` closes the channel; the session loop ends
            // and emits Disconnected into the (about to be dropped) rx.
            drop(state);
        }
    }

    pub(crate) fn process_urp_updates(&mut self, el: &ActiveEventLoop) {
        let spool_dialog = {
            let Some(ref mut state) = self.urp else {
                return;
            };
            state.drain_keys();
            while let Ok(event) = state.events_rx.try_recv() {
                match handle_urp_event(state, event) {
                    UrpAction::Continue => {
                        state.window.request_redraw();
                    }
                    UrpAction::Disconnect => {
                        self.stop.trigger();
                        self.return_code = crate::types::ReturnCode::Exit;
                        self.close_urp();
                        el.exit();
                        return;
                    }
                }
            }
            // The worker thread is gone without a Disconnected event (panic).
            if let Ok(result) = state.result_rx.try_recv() {
                if let Err(e) = result {
                    log::error!("URP worker died: {e}");
                }
                self.stop.trigger();
                self.return_code = crate::types::ReturnCode::Exit;
                self.close_urp();
                el.exit();
                return;
            }
            let pending = state.spool_dialog_pending;
            state.spool_dialog_pending = false;
            pending
        };
        if spool_dialog {
            self.open_urp_spool_dialog(el);
        }
        self.poll_urp_spool_reply();
    }

    /// Opens the spool picker dialog with the entries of the last
    /// `StorageList` event.
    fn open_urp_spool_dialog(&mut self, el: &ActiveEventLoop) {
        let entries = match self.urp {
            Some(ref mut s) => std::mem::take(&mut s.spool_entries),
            None => return,
        };
        if entries.is_empty() {
            log::info!("URP remote spool is empty; nothing to download");
            return;
        }
        let choices: Vec<String> = entries
            .iter()
            .map(|(n, sz)| format!("{n} ({sz} bytes)"))
            .collect();
        let spec = DialogSpec {
            title: "Download from remote".to_string(),
            fields: vec![DialogField {
                name: "file".to_string(),
                label: Some("Spool file".to_string()),
                size: None,
                field_type: DialogFieldType::Choice,
                value: String::new(),
                choices: Some(choices),
            }],
            buttons: vec![
                DialogButton {
                    name: "download".to_string(),
                    label: "Download".to_string(),
                    close_on_click: true,
                    primary: true,
                },
                DialogButton {
                    name: "cancel".to_string(),
                    label: "Cancel".to_string(),
                    close_on_click: true,
                    primary: false,
                },
            ],
            width: None,
            height: None,
        };
        let (tx, rx) = oneshot::channel();
        let resp = Arc::new(RwLock::new(Some(tx)));
        match DialogState::new(el, spec, resp) {
            Ok(d) => {
                let wid = d.window.id();
                self.register_window(wid, WindowKind::Dialog);
                d.window.set_visible(true);
                d.window.request_redraw();
                self.dialog = Some(d);
                if let Some(ref mut s) = self.urp {
                    s.spool_entries = entries;
                    s.spool_reply = Some(rx);
                }
            }
            Err(e) => log::warn!("URP spool dialog failed: {e}"),
        }
    }

    /// Polls the spool picker answer; on "download", asks for the local
    /// destination with a native save dialog and starts the transfer.
    fn poll_urp_spool_reply(&mut self) {
        enum Poll {
            Result(Box<DialogResult>),
            Closed,
            Pending,
        }
        let poll = {
            let Some(ref mut state) = self.urp else {
                return;
            };
            let Some(ref mut rx) = state.spool_reply else {
                return;
            };
            match rx.try_recv() {
                Ok(r) => Poll::Result(Box::new(r)),
                Err(oneshot::error::TryRecvError::Empty) => Poll::Pending,
                Err(oneshot::error::TryRecvError::Closed) => Poll::Closed,
            }
        };
        match poll {
            Poll::Pending => {}
            Poll::Closed => {
                // Dialog window closed without an answer.
                if let Some(ref mut s) = self.urp {
                    s.spool_reply = None;
                }
            }
            Poll::Result(result) => {
                let Some(ref mut state) = self.urp else {
                    return;
                };
                state.spool_reply = None;
                if result.pushed.as_deref() != Some("download") {
                    return;
                }
                let name = result
                    .values
                    .get("file")
                    .and_then(|chosen| {
                        state
                            .spool_entries
                            .iter()
                            .find(|(n, sz)| format!("{n} ({sz} bytes)") == *chosen)
                            .map(|(n, _)| n.clone())
                    })
                    .or_else(|| state.spool_entries.first().map(|(n, _)| n.clone()));
                let Some(name) = name else {
                    return;
                };
                let commands = state.commands.clone();
                let spawned = std::thread::Builder::new()
                    .name("uds-rp-download-pick".to_string())
                    .spawn(move || {
                        if let Some(dest) = rfd::FileDialog::new().set_file_name(&name).save_file()
                        {
                            let _ =
                                commands.try_send(ClientCommand::StorageDownload { name, dest });
                        }
                    });
                if let Err(e) = spawned {
                    log::warn!("URP download picker thread failed: {e}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dest_rect_is_always_the_full_window() {
        assert_eq!(dest_rect((800, 600)), (0.0, 0.0, 800.0, 600.0));
    }

    #[test]
    fn normalize_pointer_maps_dest_rect_to_full_range() {
        let dest = (40.0, 60.0, 1920.0, 1080.0);
        assert_eq!(normalize_pointer((40.0, 60.0), dest), (0, 0));
        assert_eq!(normalize_pointer((1960.0, 1140.0), dest), (65535, 65535));
        let (cx, cy) = normalize_pointer((1000.0, 600.0), dest);
        assert!((32700..=32835).contains(&cx) && (32700..=32835).contains(&cy));
    }

    #[test]
    fn normalize_pointer_clamps_outside_dest() {
        let dest = (40.0, 60.0, 1920.0, 1080.0);
        assert_eq!(normalize_pointer((0.0, 0.0), dest), (0, 0));
        assert_eq!(normalize_pointer((10000.0, 10000.0), dest), (65535, 65535));
    }
}
