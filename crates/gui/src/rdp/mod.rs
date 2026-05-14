// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

mod cursor;
mod fps;
mod pinbar;
mod rail;
mod session;

pub use cursor::Cursor;
pub use fps::Fps;
pub use pinbar::Pinbar;
pub use rail::{RailAction, RailState, RailWindow};

use std::collections::HashMap;
use std::sync::{Arc, RwLock, atomic::AtomicBool};

use anyhow::Result;
use flume::{Receiver, bounded};
use rdp_ffi::messaging::RdpMessage;
use rdp_ffi::settings::RdpSettings;
use shared::log;

use crate::RawKey;

const FRAMES_IN_FLIGHT: usize = 128;

#[allow(dead_code)]
pub struct RdpWindow {
    pub window: Arc<winit::window::Window>,
    pub renderer: crate::wgpu_render::WgpuRenderer,
    pub scratch: Vec<u8>,
}
#[allow(dead_code)]
pub struct RdpState {
    pub window: RdpWindow,
    pub update_rx: Receiver<RdpMessage>,
    pub gdi: *mut rdp_ffi::sys::rdpGdi,
    pub gdi_lock: Arc<RwLock<()>>,
    pub channels: Arc<RwLock<rdp_ffi::channels::RdpChannels>>,
    pub command_tx: rdp_ffi::commands::Sender,
    pub command_event: rdp_ffi::utils::SafeHandle,
    pub is_rail: bool,
    pub coords_scale: f64,
    pub desktop_size: (u32, u32),
    pub full_screen: Arc<AtomicBool>,
    pub last_windowed_size: Option<(u32, u32)>,
    pub last_resize: std::time::Instant,
    pub pending_resize: bool,
    pub pinbar: Pinbar,
    pub keys_rx: Receiver<RawKey>,
    pub fps: Fps,

    pub rail: RailState,
    pub rail_channel: Option<rdp_ffi::channels::rail::RailChannel>,
    pub rail_actions: Vec<RailAction>,
    pub rail_windows: HashMap<u32, RailWindow>, // id → RailWindow

    pub cursor: Cursor,
    pub pending_pixels: HashMap<u32, (u32, u32, Vec<u8>)>,
}

#[allow(dead_code)]
pub enum RdpActionResult {
    Continue,
    Skip,
    Disconnect,
    Error(String),
}

impl RdpState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        window: RdpWindow,
        settings: RdpSettings,
        is_rail: bool,
        coords_scale: f64,
        desktop_size: (u32, u32),
        keys_rx: Receiver<RawKey>,
        use_rgba: bool,
    ) -> Result<Self> {
        log::debug!(
            "RdpState::new: is_rail={}, coords_scale={}, desktop_size={:?}, use_rgba={}",
            is_rail,
            coords_scale,
            desktop_size,
            use_rgba
        );
        let (tx, rx) = bounded::<RdpMessage>(FRAMES_IN_FLIGHT);

        let scale_factor = settings.scale_factor;

        let (mut rdp_instance, command_tx) = rdp_ffi::Rdp::new(settings, tx, use_rgba);
        let command_event = rdp_instance.get_command_event();

        if is_rail {
            rdp_instance.set_window_callbacks(vec![
                rdp_ffi::callbacks::window_c::Callbacks::Create,
                rdp_ffi::callbacks::window_c::Callbacks::Update,
                rdp_ffi::callbacks::window_c::Callbacks::Delete,
            ]);
        }

        let mut rdp = Box::pin(rdp_instance);
        rdp.as_mut().build()?;
        rdp.connect()?;

        let gdi = rdp
            .gdi()
            .ok_or_else(|| anyhow::anyhow!("GDI not initialized"))?;
        let gdi_lock = rdp.gdi_lock();
        let channels = rdp.channels().clone();

        log::info!(
            "RDP connected: GDI={}x{}, scale={}, stride={}",
            unsafe { (*gdi).width },
            unsafe { (*gdi).height },
            scale_factor,
            unsafe { (*gdi).stride }
        );

        let rail_channel = if is_rail {
            channels.read().unwrap().rail()
        } else {
            None
        };

        std::thread::spawn(move || {
            connection::tasks::mark_internal_rdp_as_running();
            let res = rdp.run();
            connection::tasks::mark_internal_rdp_as_not_running();
            if let Err(e) = res {
                log::error!("RDP thread error: {}", e);
            }
        });

        Ok(RdpState {
            window,
            update_rx: rx,
            gdi,
            gdi_lock,
            channels,
            command_tx,
            command_event,
            is_rail,
            coords_scale,
            desktop_size,
            full_screen: Arc::new(AtomicBool::new(false)),
            last_windowed_size: None,
            last_resize: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(60))
                .unwrap_or(std::time::Instant::now()),
            pending_resize: false,
            pinbar: Pinbar::new(),
            keys_rx,
            fps: Fps::new(),
            rail: RailState {
                windows: HashMap::new(),
                mouse_capture: None,
            },
            rail_channel,
            rail_actions: Vec::new(),
            rail_windows: HashMap::new(),
            cursor: Cursor::new(coords_scale),
            pending_pixels: HashMap::new(),
        })
    }
}

/// Process an RDP message, returning the action to take
pub fn handle_rdp_message(state: &mut RdpState, message: RdpMessage) -> RdpActionResult {
    log::trace!("RDP message: {:?}", message);
    match message {
        RdpMessage::UpdateRects(_rects) => RdpActionResult::Continue,
        RdpMessage::DesktopResize(w, h) => {
            state.on_desktop_resize(w, h);
            RdpActionResult::Continue
        }
        RdpMessage::Disconnect => RdpActionResult::Disconnect,
        RdpMessage::Error(e) => {
            log::error!("RDP Error: {}", e);
            RdpActionResult::Error(e)
        }
        RdpMessage::SetCursorIcon(data, x, y, width, height) => {
            state.cursor.set_icon(data, x, y, width, height);
            RdpActionResult::Continue
        }
        // Delegate RAIL-specific messages
        ref msg if state.is_rail => rail::handle_rail_message(state, msg.clone()),
        _ => RdpActionResult::Skip,
    }
}
