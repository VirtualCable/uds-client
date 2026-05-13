use std::collections::HashMap;

use rdp_ffi::messaging::RdpMessage;

// ── RAIL Window ─────────────────────────────────────────────

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
    Create(u32, String, rdp_ffi::geom::Rect, bool, bool),
    Delete(u32),
    UpdatePosition(u32, rdp_ffi::geom::Rect),
}

// ── RAIL Message Dispatcher ─────────────────────────────────
// Called from handle_rdp_message in mod.rs

use super::{RdpActionResult, RdpState};

pub fn handle_rail_message(state: &mut RdpState, message: RdpMessage) -> RdpActionResult {
    match message {
        RdpMessage::WindowCreate {
            window_id,
            owner_id,
            title,
            pos,
            size,
            taskbar_button,
            ext_style,
            is_offscreen,
            show_state,
            ..
        } => {
            if ext_style.is_some_and(|s| (s & 0x20) != 0) {
                return RdpActionResult::Continue;
            }
            let sf = state.coords_scale.max(1.0);
            let (x, y) = pos.unwrap_or((0, 0));
            let (w, h) = size.unwrap_or((0, 0));
            let rect = rdp_ffi::geom::Rect::new(
                (x as f64 / sf) as i32,
                (y as f64 / sf) as i32,
                (w as f64 / sf) as u32,
                (h as f64 / sf) as u32,
            );
            let is_tool = ext_style.is_some_and(|s| (s & 0x80) != 0);
            let has_owner = owner_id.is_some() && owner_id != Some(0);
            let show_taskbar = taskbar_button.unwrap_or(!is_tool && !has_owner);
            let hidden = show_state == Some(0);
            if !hidden && rect.w > 0 && rect.h > 0 && !is_offscreen.unwrap_or(false) {
                state.rail_actions.push(RailAction::Create(
                    window_id,
                    title,
                    rect,
                    show_taskbar,
                    false,
                ));
            }
            RdpActionResult::Continue
        }
        RdpMessage::WindowUpdate {
            window_id,
            pos,
            size,
            is_offscreen,
            show_state,
            ..
        } => {
            if is_offscreen.unwrap_or(false) || show_state == Some(0) {
                state.rail_actions.push(RailAction::Delete(window_id));
            } else if pos.is_some() || size.is_some() {
                let sf = state.coords_scale.max(1.0);
                let default_rect = state
                    .rail_windows
                    .get(&window_id)
                    .map(|rw| rw.rect)
                    .unwrap_or(rdp_ffi::geom::Rect::new(0, 0, 0, 0));
                let x = match pos {
                    Some((x, _)) => (x as f64 / sf) as i32,
                    None => default_rect.x,
                };
                let y = match pos {
                    Some((_, y)) => (y as f64 / sf) as i32,
                    None => default_rect.y,
                };
                let w = match size {
                    Some((w, _)) => (w as f64 / sf) as u32,
                    None => default_rect.w,
                };
                let h = match size {
                    Some((_, h)) => (h as f64 / sf) as u32,
                    None => default_rect.h,
                };
                let rect = rdp_ffi::geom::Rect::new(x, y, w, h);
                state
                    .rail_actions
                    .push(RailAction::UpdatePosition(window_id, rect));
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
                    let _ = rw.window.request_inner_size(winit::dpi::LogicalSize::new(
                        lw as f64,
                        lh as f64,
                    ));
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
