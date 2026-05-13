use std::collections::HashMap;

use shared::log;
use rdp_ffi::messaging::RdpMessage;

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
}

#[allow(dead_code)]
pub struct RailState {
    pub windows: HashMap<winit::window::WindowId, u32>,
    pub mouse_capture: Option<u32>,
}

/// Pending RAIL action to be executed by the event loop
pub enum RailAction {
    Create(u32, String, rdp_ffi::geom::Rect, bool, bool, bool),
    Delete(u32),
    UpdatePosition(u32, rdp_ffi::geom::Rect),
    SetVisible(u32, bool),
}

// ── RAIL Message Dispatcher ─────────────────────────────────
// Called from handle_rdp_message in mod.rs

use super::{RdpActionResult, RdpState};

pub fn handle_rail_message(state: &mut RdpState, message: RdpMessage) -> RdpActionResult {
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
                log::debug!("Skipping RAIL window {} with style 0x20 (probably a menu)", window_id);
                return RdpActionResult::Continue;
            }
            let sf = state.coords_scale.max(1.0);
            let is_off = is_offscreen.unwrap_or(false);

            let mut exists = state.rail_windows.contains_key(&window_id);
            if !exists {
                for action in &state.rail_actions {
                    if let RailAction::Create(id, ..) = action
                        && *id == window_id
                    {
                        exists = true;
                        break;
                    }
                }
            }

            if exists {
                if let Some(s) = show_state {
                    let hidden = s == 0 || is_off;
                    state
                        .rail_actions
                        .push(RailAction::SetVisible(window_id, !hidden));
                } else if is_offscreen.is_some() {
                    state
                        .rail_actions
                        .push(RailAction::SetVisible(window_id, !is_off));
                }

                if pos.is_some() || size.is_some() {
                    let default_rect = state
                        .rail_windows
                        .get(&window_id)
                        .map(|rw| rw.rect)
                        .unwrap_or_else(|| rdp_ffi::geom::Rect::new(0, 0, 0, 0));
                    let mut rect = default_rect;
                    if !state.rail_windows.contains_key(&window_id) {
                        for action in &state.rail_actions {
                            if let RailAction::Create(id, _, r, ..) = action
                                && *id == window_id
                            {
                                rect = *r;
                            }
                        }
                    }

                    let x = match pos {
                        Some((x, _)) => (x as f64 / sf) as i32,
                        None => rect.x,
                    };
                    let y = match pos {
                        Some((_, y)) => (y as f64 / sf) as i32,
                        None => rect.y,
                    };
                    let w = match size {
                        Some((w, _)) => (w as f64 / sf) as u32,
                        None => rect.w,
                    };
                    let h = match size {
                        Some((_, h)) => (h as f64 / sf) as u32,
                        None => rect.h,
                    };
                    let new_rect = rdp_ffi::geom::Rect::new(x, y, w, h);
                    state
                        .rail_actions
                        .push(RailAction::UpdatePosition(window_id, new_rect));
                }
            } else {
                let (w, h) = size.unwrap_or((0, 0));
                if w > 0 && h > 0 {
                    let (x, y) = pos.unwrap_or((0, 0));
                    let rect = rdp_ffi::geom::Rect::new(
                        (x as f64 / sf) as i32,
                        (y as f64 / sf) as i32,
                        (w as f64 / sf) as u32,
                        (h as f64 / sf) as u32,
                    );
                    let is_tool = ext_style.is_some_and(|s| (s & 0x80) != 0);
                    let has_owner = owner_id.is_some() && owner_id != Some(0);
                    let show_taskbar = taskbar_button.unwrap_or(!is_tool && !has_owner);

                    let hidden = show_state == Some(0) || is_off;

                    state.rail_actions.push(RailAction::Create(
                        window_id,
                        title,
                        rect,
                        show_taskbar,
                        false,
                        !hidden,
                    ));
                }
            }

            RdpActionResult::Continue
        }
        RdpMessage::WindowDelete(window_id) => {
            state.rail_actions.push(RailAction::Delete(window_id));
            RdpActionResult::Continue
        }
        RdpMessage::WindowPixels {
            window_id,
            width,
            height,
            data,
        } => {
            if let Some(rw) = state.rail_windows.get_mut(&window_id) {
                let sf = state.coords_scale.max(1.0);
                let lw = ((width as f64 / sf) as u32).min(state.desktop_size.0);
                let lh = ((height as f64 / sf) as u32).min(state.desktop_size.1);
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
                rw.window.request_redraw();
            }
            RdpActionResult::Continue
        }
        _ => RdpActionResult::Skip,
    }
}
