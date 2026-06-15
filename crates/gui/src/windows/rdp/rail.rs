use std::collections::HashMap;

use rdp_ffi::messaging::RdpMessage;
use shared::log;

#[allow(dead_code)]
pub struct RailWindow {
    pub id: u32,
    pub window: std::sync::Arc<winit::window::Window>,
    pub renderer: Option<crate::wgpu_render::WgpuRenderer>,
    pub rgba_data: Option<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub rect: rdp_ffi::geom::Rect,
    pub title: String,
    pub show_in_taskbar: bool,
    pub has_decorations: bool,
    pub last_focused: bool,
    pub offscreen: bool,
    pub was_minimized: bool,
    pub server_minimized: bool,
    pub rgba_dirty: bool,
}

#[allow(dead_code)]
pub struct RailState {
    pub windows: HashMap<winit::window::WindowId, u32>,
    pub mouse_capture: Option<u32>,
}

/// Pending RAIL action to be executed by the event loop
pub enum RailAction {
    Create(u32, String, rdp_ffi::geom::Rect, bool, bool, bool, bool),
    Delete(u32),
    UpdatePosition(u32, rdp_ffi::geom::Rect),
    SetVisible(u32, bool),
    SetMinimized(u32, bool),
}

// ── RAIL Message Dispatcher ───
// Called from handle_rdp_message in mod.rs

use super::{RdpActionResult, RdpMode, RdpState};

#[allow(clippy::unnecessary_cast)]
pub fn handle_rail_message(state: &mut RdpState, message: RdpMessage) -> RdpActionResult {
    let RdpMode::Rail(ref mut rail) = state.mode else {
        return RdpActionResult::Skip;
    };
    match message {
        RdpMessage::WindowCreate {
            window_id,
            owner_id,
            style: _,
            ext_style,
            taskbar_button,
            title,
            show_state,
            is_offscreen,
            pos,
            size,
        }
        | RdpMessage::WindowUpdate {
            window_id,
            owner_id,
            style: _,
            ext_style,
            taskbar_button,
            title,
            show_state,
            is_offscreen,
            pos,
            size,
        } => {
            if ext_style.is_some_and(|s| (s & 0x20) != 0) {
                log::debug!(
                    "Skipping RAIL window {} with style 0x20 (probably a menu)",
                    window_id
                );
                return RdpActionResult::Continue;
            }
            let sf = state.coords_scale.max(1.0);
            let msf = state.window.window.scale_factor().max(1.0);
            let is_off = is_offscreen.unwrap_or(false);

            let mut exists = rail.windows.contains_key(&window_id);
            if !exists {
                for action in &rail.actions {
                    if let RailAction::Create(id, ..) = action
                        && *id == window_id
                    {
                        exists = true;
                        break;
                    }
                }
            }

            if exists {
                let is_minimized = show_state.is_some_and(|s| {
                    s == 2 || s == 6 || s == 7 || s == 11
                });
                if let Some(s) = show_state {
                    let hidden = s == 0 || (is_off && !is_minimized);
                    // Track server-side minimized state so offscreen updates don't undo it
                    if let Some(rw) = rail.windows.get_mut(&window_id) {
                        rw.server_minimized = is_minimized;
                    }
                    rail.actions
                        .push(RailAction::SetVisible(window_id, !hidden));
                    if is_minimized {
                        rail.actions.push(RailAction::SetMinimized(window_id, true));
                    } else if s == 1 || s == 3 || s == 5 || s == 9 {
                        rail.actions.push(RailAction::SetMinimized(window_id, false));
                    }
                } else if let Some(is_off_val) = is_offscreen {
                    // If the server told us this window is minimized, ignore offscreen
                    // updates — the (-32000,-32000) position is expected.
                    let server_minimized = rail.windows.get(&window_id)
                        .is_some_and(|rw| rw.server_minimized);
                    if !server_minimized {
                        if !is_off_val {
                            rail.actions.push(RailAction::SetMinimized(window_id, false));
                        }
                        rail.actions
                            .push(RailAction::SetVisible(window_id, !is_off_val));
                    }
                }

                if pos.is_some() || size.is_some() {
                    let default_rect = rail
                        .windows
                        .get(&window_id)
                        .map(|rw| rw.rect)
                        .unwrap_or_else(|| rdp_ffi::geom::Rect::new(0, 0, 0, 0));
                    let mut rect = default_rect;
                    if !rail.windows.contains_key(&window_id) {
                        for action in &rail.actions {
                            if let RailAction::Create(id, _, r, ..) = action
                                && *id == window_id
                            {
                                rect = *r;
                            }
                        }
                    }

                    let x = match pos {
                        Some((x, _)) => x,
                        None => rect.x,
                    };
                    let y = match pos {
                        Some((_, y)) => y,
                        None => rect.y,
                    };
                    let w = match size {
                        Some((w, _)) => (w as f64 * sf / msf) as u32,
                        None => rect.w,
                    };
                    let h = match size {
                        Some((_, h)) => (h as f64 * sf / msf) as u32,
                        None => rect.h,
                    };
                    let new_rect = rdp_ffi::geom::Rect::new(x, y, w, h);
                    rail.actions
                        .push(RailAction::UpdatePosition(window_id, new_rect));
                }
            } else {
                let (w, h) = size.unwrap_or((0, 0));
                if w > 0 && h > 0 {
                    let (x, y) = pos.unwrap_or((0, 0));
                    let rect = rdp_ffi::geom::Rect::new(
                        x as i32,
                        y as i32,
                        (w as f64 * sf / msf) as u32,
                        (h as f64 * sf / msf) as u32,
                    );
                    let is_tool = ext_style.is_some_and(|s| (s & 0x80) != 0);
                    let has_owner = owner_id.is_some() && owner_id != Some(0);
                    let show_taskbar = taskbar_button.unwrap_or(false) || (!is_tool && !has_owner);

                    let is_minimized = show_state.is_some_and(|s| {
                        s == 2 || s == 6 || s == 7 || s == 11
                    });
                    let hidden = show_state == Some(0) || (is_off && !is_minimized);

                    rail.actions.push(RailAction::Create(
                        window_id,
                        title,
                        rect,
                        show_taskbar,
                        false,
                        !hidden,
                        is_minimized,
                    ));
                }
            }

            RdpActionResult::Continue
        }
        RdpMessage::WindowDelete(window_id) => {
            rail.actions.push(RailAction::Delete(window_id));
            RdpActionResult::Continue
        }
        RdpMessage::WindowPixels {
            window_id,
            width,
            height,
            data,
        } => {
            log::debug!(
                "WindowPixels: {}x{} for window {}",
                width,
                height,
                window_id
            );
            if let Some(rw) = rail.windows.get_mut(&window_id) {
                let sf = state.coords_scale.max(1.0);
                let msf = rw.window.scale_factor().max(1.0);
                let lw = ((width as f64 * sf / msf) as u32).min(state.desktop_size.0);
                let lh = ((height as f64 * sf / msf) as u32).min(state.desktop_size.1);
                if rw.rect.w != lw || rw.rect.h != lh {
                    rw.rect.w = lw;
                    rw.rect.h = lh;
                    let _ = rw
                        .window
                        .request_inner_size(winit::dpi::LogicalSize::new(lw as f64, lh as f64));
                    if let Some(ref mut renderer) = rw.renderer {
                        let phys = rw.window.inner_size();
                        renderer.reconfigure(phys.width, phys.height);
                    }
                }
                rw.rgba_data = Some(data);
                rw.width = width;
                rw.height = height;
                rw.rgba_dirty = true;
                rw.window.request_redraw();
            } else {
                // Window not created yet — buffer pixels for when RailAction::Create is processed
                state
                    .pendings
                    .pixels
                    .insert(window_id, (width, height, data));
            }
            RdpActionResult::Continue
        }
        RdpMessage::WindowIcon {
            window_id,
            rgba,
            width,
            height,
        } => {
            if let Some(rw) = rail.windows.get(&window_id) {
                if let Ok(icon) = winit::window::Icon::from_rgba(rgba, width, height) {
                    rw.window.set_window_icon(Some(icon));
                }
            } else {
                // Buffer icon for pending window (same pattern as pending_pixels)
                state
                    .pendings
                    .icons
                    .insert(window_id, (rgba, width, height));
            }
            RdpActionResult::Continue
        }
        _ => RdpActionResult::Skip,
    }
}
