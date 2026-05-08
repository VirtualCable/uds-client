// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use crate::callbacks::window::WindowCallbacks;
use crate::consts::*;
use crate::messaging::RdpMessage;
use freerdp_sys::{
    WINDOW_ORDER_FIELD_SHOW, WINDOW_ORDER_FIELD_WND_OFFSET, WINDOW_ORDER_FIELD_WND_SIZE, WINDOW_ORDER_INFO, WINDOW_STATE_ORDER,
};
use shared::log;

use super::Rdp;

fn get_rail_string(rail_str: &freerdp_sys::RAIL_UNICODE_STRING) -> String {
    if rail_str.string.is_null() || rail_str.length == 0 {
        return String::new();
    }
    let len = rail_str.length as usize / 2;
    let slice = unsafe { std::slice::from_raw_parts(rail_str.string as *const u16, len) };
    String::from_utf16_lossy(slice)
}

/// Windows convention: minimized windows are moved to (-32000, -32000).
/// Anything below OFFSCREEN_THRESHOLD on either axis is considered offscreen/minimized.
fn is_offscreen_pos(x: i32, y: i32) -> bool {
    x < OFFSCREEN_THRESHOLD || y < OFFSCREEN_THRESHOLD
}

impl WindowCallbacks for Rdp {
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn on_window_create(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        window_state: *const WINDOW_STATE_ORDER,
    ) -> bool {
        let (window_id, title, show_state, is_offscreen, pos, size) = unsafe {
            let info = &*order_info;
            let state = &*window_state;
            let show_state = if info.fieldFlags & WINDOW_ORDER_FIELD_SHOW != 0 {
                Some(state.showState)
            } else {
                None
            };
            let is_offscreen = if info.fieldFlags & WINDOW_ORDER_FIELD_WND_OFFSET != 0 {
                Some(is_offscreen_pos(state.windowOffsetX, state.windowOffsetY))
            } else {
                None
            };
            let pos = if info.fieldFlags & WINDOW_ORDER_FIELD_WND_OFFSET != 0 {
                Some((state.windowOffsetX as i32, state.windowOffsetY as i32))
            } else {
                None
            };
            let size = if info.fieldFlags & WINDOW_ORDER_FIELD_WND_SIZE != 0 {
                Some((state.windowWidth as u32, state.windowHeight as u32))
            } else {
                None
            };
            (
                info.windowId,
                get_rail_string(&state.titleInfo),
                show_state,
                is_offscreen,
                pos,
                size,
            )
        };

        log::debug!(
            "WindowCreate: id={}, title={:?}, show_state={:?}, is_offscreen={:?}, pos={:?}, size={:?}",
            window_id,
            title,
            show_state,
            is_offscreen,
            pos,
            size
        );

        if let Some(tx) = &self.update_tx {
            let _ = tx.send(RdpMessage::WindowCreate {
                window_id,
                title,
                show_state,
                is_offscreen,
                pos,
                size,
            });
            // If the window is being created in SW_SHOW(5) or SW_SHOWMAXIMIZED(3) state,
            // trigger a screen sync to be safe.
            if show_state == Some(SW_SHOWMAXIMIZED) || show_state == Some(SW_SHOW) {
                let _ = tx.send(RdpMessage::UpdateRects(vec![crate::geom::Rect::new(
                    0,
                    0,
                    MAX_SYNC_SIZE,
                    MAX_SYNC_SIZE,
                )]));
            }
        }
        true
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn on_window_update(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        window_state: *const WINDOW_STATE_ORDER,
    ) -> bool {
        let (window_id, title, show_state, is_offscreen, pos, size, state_ref) = unsafe {
            let info = &*order_info;
            let state = &*window_state;
            let show_state = if info.fieldFlags & WINDOW_ORDER_FIELD_SHOW != 0 {
                Some(state.showState)
            } else {
                None
            };
            let is_offscreen = if info.fieldFlags & WINDOW_ORDER_FIELD_WND_OFFSET != 0 {
                Some(is_offscreen_pos(state.windowOffsetX, state.windowOffsetY))
            } else {
                None
            };
            let pos = if info.fieldFlags & WINDOW_ORDER_FIELD_WND_OFFSET != 0 {
                Some((state.windowOffsetX as i32, state.windowOffsetY as i32))
            } else {
                None
            };
            let size = if info.fieldFlags & WINDOW_ORDER_FIELD_WND_SIZE != 0 {
                Some((state.windowWidth as u32, state.windowHeight as u32))
            } else {
                None
            };
            (
                info.windowId,
                get_rail_string(&state.titleInfo),
                show_state,
                is_offscreen,
                pos,
                size,
                state,
            )
        };

        log::debug!(
            "WindowUpdate: id={}, flags=0x{:X}, title={:?}, show_state={:?}, offset=({}, {}), offscreen={:?}, pos={:?}, size={:?}",
            window_id,
            unsafe { (*order_info).fieldFlags },
            title,
            show_state,
            state_ref.windowOffsetX,
            state_ref.windowOffsetY,
            is_offscreen,
            pos,
            size
        );

        if let Some(tx) = &self.update_tx {
            let _ = tx.send(RdpMessage::WindowUpdate {
                window_id,
                title,
                show_state,
                is_offscreen,
                pos,
                size,
            });

            if let Some(show_state) = show_state
                && [SW_SHOW, SW_SHOWNORMAL].contains(&show_state)
                && is_offscreen == Some(false)
            {
                log::info!("RAIL: Restoration nudge triggered for window {}", window_id);
                let _ = tx.send(RdpMessage::FocusRequired);
                let _ = tx.send(RdpMessage::UpdateRects(vec![crate::geom::Rect::new(
                    0,
                    0,
                    MAX_SYNC_SIZE,
                    MAX_SYNC_SIZE,
                )]));
            }
        }
        true
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn on_window_delete(&self, order_info: *const WINDOW_ORDER_INFO) -> bool {
        let window_id = unsafe { (*order_info).windowId };
        log::debug!("WindowDelete: id={}", window_id);
        if let Some(tx) = &self.update_tx {
            let _ = tx.send(RdpMessage::WindowDelete(window_id));
        }
        true
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn on_monitored_desktop(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        _monitored_desktop: *const freerdp_sys::MONITORED_DESKTOP_ORDER,
    ) -> bool {
        let flags = unsafe { (*order_info).fieldFlags };
        log::debug!(
            "RAIL: on_monitored_desktop called, fieldFlags: 0x{:08X}",
            flags
        );

        true
    }
}
