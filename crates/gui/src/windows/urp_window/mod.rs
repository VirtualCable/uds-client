// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

//! URP (UDS Remote Protocol) session window: drives a
//! `uds_rp::client::Session` from the launcher's winit loop, rendering the
//! shared display framebuffer with the two-pass XOR cursor (protocol §10)
//! and forwarding input as Set-1 scancodes + normalized pointer moves.
//!
//! Scaling policy (per the transport settings): the remote resolution is
//! fixed at connect time (the protocol has no display-mode renegotiation
//! yet). With `allow_fullscreen_scaling` the image stretches to the window;
//! otherwise it is drawn 1:1 (shrinking only when the window is smaller),
//! centered with black bars.

use std::net::ToSocketAddrs;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Result;
use flume::Receiver;
use rdp::integrations::ClipboardIntegration;
use shared::log;
use uds_rp::client::{
    ClientCommand, ClientEvent, CommandSender, Displays, EVENT_CHANNEL_CAPACITY, Session,
    SessionCredentials,
};
use uds_rp::core::InputMessage;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;

use crate::altgr::{AltGr, CtrlAction};
use crate::types::UrpSettings;
use crate::wgpu_render::{CursorSprite, WgpuRenderer};
use crate::{AppHandler, RawKey, WindowKind};

/// Default cap for the auto (monitor-derived) remote resolution.
const DEFAULT_MAX_FULLSCREEN: (u32, u32) = (1920, 1080);

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
    /// Scratch buffer for packed dirty-rect uploads.
    pub scratch: Vec<u8>,
    /// Last known local pointer position (physical window pixels).
    pub pointer: (f64, f64),
    /// Current remote cursor sprite; None until the first shape arrives.
    pub cursor: Option<CursorSprite>,
    /// Whether the remote cursor is visible (server-driven, §10).
    pub cursor_visible: bool,
    pub fullscreen: bool,
    pub allow_scaling: bool,
    /// True while the remote console is on a secure desktop (§18.41).
    pub secure_desktop: bool,
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

/// Surface-pixel rect the desktop image occupies (letterboxed when the
/// remote image is not allowed to stretch).
#[expect(clippy::cast_precision_loss, reason = "window sizes fit in f32")]
pub fn dest_rect(win: (u32, u32), disp: (u32, u32), allow_scaling: bool) -> (f32, f32, f32, f32) {
    let (ww, wh) = (win.0 as f32, win.1 as f32);
    if allow_scaling || disp.0 == 0 || disp.1 == 0 || ww <= 0.0 || wh <= 0.0 {
        return (0.0, 0.0, ww, wh);
    }
    let s = (ww / disp.0 as f32).min(wh / disp.1 as f32).min(1.0);
    let (w, h) = (disp.0 as f32 * s, disp.1 as f32 * s);
    ((ww - w) / 2.0, (wh - h) / 2.0, w, h)
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

    /// Sends the current pointer position to the server.
    pub(crate) fn send_pointer_move(&self) {
        let size = self.window.inner_size();
        let dest = dest_rect(
            (size.width, size.height),
            self.desktop_size,
            self.allow_scaling,
        );
        let (x, y) = normalize_pointer(self.pointer, dest);
        let _ = self
            .commands
            .try_send(ClientCommand::Input(InputMessage::PointerMove {
                display_id: 0,
                x,
                y,
            }));
    }

    /// Renders one frame: uploads pending damage from the shared framebuffer
    /// and presents it with the software cursor on top.
    pub fn update_screen(&mut self) {
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
        let dest = dest_rect((size.width, size.height), (w, h), self.allow_scaling);
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

        let cap = (
            if settings.max_fullscreen_width > 0 {
                settings.max_fullscreen_width
            } else {
                DEFAULT_MAX_FULLSCREEN.0
            },
            if settings.max_fullscreen_height > 0 {
                settings.max_fullscreen_height
            } else {
                DEFAULT_MAX_FULLSCREEN.1
            },
        );
        let monitor_scale = crate::monitor::scale(0);
        let (mon_w, mon_h) = crate::monitor::size(0).unwrap_or(DEFAULT_MAX_FULLSCREEN);
        let (mon_lw, mon_lh) =
            crate::monitor::phys_2_logic((mon_w as i32, mon_h as i32), monitor_scale);
        let fixed = settings.screen_width > 0 && settings.screen_height > 0;
        let (remote_w, remote_h) = if fixed {
            (settings.screen_width, settings.screen_height)
        } else {
            (
                (mon_lw as u32).clamp(1, cap.0),
                (mon_lh as u32).clamp(1, cap.1),
            )
        };
        let pref_resolution = (
            u16::try_from(remote_w).unwrap_or(u16::MAX),
            u16::try_from(remote_h).unwrap_or(u16::MAX),
        );

        let window = Arc::new(
            el.create_window(
                winit::window::Window::default_attributes()
                    .with_visible(false)
                    .with_title("UDS Remote Desktop")
                    .with_inner_size(winit::dpi::LogicalSize::new(
                        remote_w as f64,
                        remote_h as f64,
                    ))
                    .with_window_icon(Some(crate::logo::load_icon())),
            )?,
        );
        let wid = window.id();
        if settings.fullscreen {
            window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
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

        let config = uds_rp::client::ClientConfig {
            addr,
            ticket: settings.ticket,
            key: settings.key,
            audio: settings.redirections.audio,
            audio_dump: None,
            mic: uds_rp::client::mic::MicMode::Off,
            // Privacy screen stays off: the server ignores the flag anyway
            // (incompatible with console-mirror capture; the IddCx work is
            // archived in the uds-rp `archive/iddcx-display` branch).
            privacy_screen: false,
            // Phase 4 wires the real device adapters from `channels`.
            integrations: uds_rp::client::integrations::Integrations::default(),
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
            scratch: Vec::new(),
            pointer: (0.0, 0.0),
            cursor: None,
            cursor_visible: true,
            fullscreen: settings.fullscreen,
            allow_scaling: settings.allow_fullscreen_scaling,
            secure_desktop: false,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dest_rect_stretches_when_scaling_allowed() {
        assert_eq!(
            dest_rect((800, 600), (1920, 1080), true),
            (0.0, 0.0, 800.0, 600.0)
        );
    }

    #[test]
    fn dest_rect_centers_1to1_when_not_scaling() {
        // 1920x1080 remote on a 2000x1200 window: 1:1, centered.
        let (x, y, w, h) = dest_rect((2000, 1200), (1920, 1080), false);
        assert_eq!((w, h), (1920.0, 1080.0));
        assert_eq!((x, y), (40.0, 60.0));
    }

    #[test]
    fn dest_rect_shrinks_to_fit_smaller_window() {
        // 1920x1080 remote on a 960x600 window: fit = 960x540, centered.
        let (x, y, w, h) = dest_rect((960, 600), (1920, 1080), false);
        assert_eq!((w, h), (960.0, 540.0));
        assert_eq!((x, y), (0.0, 30.0));
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
