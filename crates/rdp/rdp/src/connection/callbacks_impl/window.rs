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
use crate::utils::log;
use freerdp_sys::{
    WINDOW_CACHED_ICON_ORDER, WINDOW_ICON_ORDER, WINDOW_ORDER_FIELD_OWNER, WINDOW_ORDER_FIELD_SHOW,
    WINDOW_ORDER_FIELD_STYLE, WINDOW_ORDER_FIELD_TASKBAR_BUTTON, WINDOW_ORDER_FIELD_WND_OFFSET,
    WINDOW_ORDER_FIELD_WND_SIZE, WINDOW_ORDER_INFO, WINDOW_STATE_ORDER,
};

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

struct RailWindowState {
    window_id: u32,
    owner_id: Option<u32>,
    style: Option<u32>,
    ext_style: Option<u32>,
    taskbar_button: Option<bool>,
    title: String,
    show_state: Option<u32>,
    is_offscreen: Option<bool>,
    pos: Option<(i32, i32)>,
    size: Option<(u32, u32)>,
}

impl RailWindowState {
    #[allow(clippy::unnecessary_cast)]
    unsafe fn from_raw(
        order_info: *const WINDOW_ORDER_INFO,
        window_state: *const WINDOW_STATE_ORDER,
    ) -> Self {
        let info = unsafe { &*order_info };
        let state = unsafe { &*window_state };

        let owner_id = if info.fieldFlags & WINDOW_ORDER_FIELD_OWNER != 0 {
            Some(state.ownerWindowId)
        } else {
            None
        };
        let (style, ext_style) = if info.fieldFlags & WINDOW_ORDER_FIELD_STYLE != 0 {
            (Some(state.style), Some(state.extendedStyle))
        } else {
            (None, None)
        };
        let taskbar_button = if info.fieldFlags & WINDOW_ORDER_FIELD_TASKBAR_BUTTON != 0 {
            Some(state.TaskbarButton != 0)
        } else {
            None
        };
        let show_state = if info.fieldFlags & WINDOW_ORDER_FIELD_SHOW != 0 {
            Some(state.showState as u32)
        } else {
            None
        };
        let (is_offscreen, pos) = if info.fieldFlags & WINDOW_ORDER_FIELD_WND_OFFSET != 0 {
            (
                Some(is_offscreen_pos(state.windowOffsetX, state.windowOffsetY)),
                Some((state.windowOffsetX as i32, state.windowOffsetY as i32)),
            )
        } else {
            (None, None)
        };
        let size = if info.fieldFlags & WINDOW_ORDER_FIELD_WND_SIZE != 0 {
            Some((state.windowWidth as u32, state.windowHeight as u32))
        } else {
            None
        };

        Self {
            window_id: info.windowId,
            owner_id,
            style,
            ext_style,
            taskbar_button,
            title: get_rail_string(&state.titleInfo),
            show_state,
            is_offscreen,
            pos,
            size,
        }
    }
}

impl WindowCallbacks for Rdp {
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn on_window_create(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        window_state: *const WINDOW_STATE_ORDER,
    ) -> bool {
        // Sanity checks
        if order_info.is_null() || window_state.is_null() {
            log::error!("WindowCallbacks::on_window_create: order_info or window_state is null");
            return false;
        }

        unsafe {
            log::trace!(
                "WindowCallbacks::on_window_create: order_info={:?}, window_state={:?}",
                *order_info,
                *window_state
            )
        };
        let rw = unsafe { RailWindowState::from_raw(order_info, window_state) };

        if let Some(tx) = &self.update_tx {
            let _ = tx.send(RdpMessage::WindowCreate {
                window_id: rw.window_id,
                owner_id: rw.owner_id,
                style: rw.style,
                ext_style: rw.ext_style,
                taskbar_button: rw.taskbar_button,
                title: rw.title.clone(),
                show_state: rw.show_state,
                is_offscreen: rw.is_offscreen,
                pos: rw.pos,
                size: rw.size,
            });
            // If the window is being created in SW_SHOW(5) or SW_SHOWMAXIMIZED(3) state,
            // trigger a screen sync to be safe.
            if rw.show_state == Some(SW_SHOWMAXIMIZED) || rw.show_state == Some(SW_SHOW) {
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
        unsafe {
            log::trace!(
                "WindowCallbacks::on_window_update: order_info={:?}, window_state={:?}",
                *order_info,
                *window_state
            )
        };

        let rw = unsafe { RailWindowState::from_raw(order_info, window_state) };

        if let Some(tx) = &self.update_tx {
            let _ = tx.send(RdpMessage::WindowUpdate {
                window_id: rw.window_id,
                owner_id: rw.owner_id,
                style: rw.style,
                ext_style: rw.ext_style,
                taskbar_button: rw.taskbar_button,
                title: rw.title,
                show_state: rw.show_state,
                is_offscreen: rw.is_offscreen,
                pos: rw.pos,
                size: rw.size,
            });

            if let Some(show_state) = rw.show_state
                && [SW_SHOW, SW_SHOWNORMAL].contains(&show_state)
                && rw.is_offscreen == Some(false)
            {
                log::info!(
                    "RAIL: Restoration nudge triggered for window {}",
                    rw.window_id
                );
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

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn on_window_icon(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        icon: *const WINDOW_ICON_ORDER,
    ) -> bool {
        let is_individual = self.config.settings.rail.as_ref().map(|r| r.behavior)
            == Some(crate::settings::RailBehavior::IndividualWindows);
        if !is_individual {
            return true;
        }

        let window_id = unsafe { (*order_info).windowId };
        if icon.is_null() || unsafe { (*icon).iconInfo.is_null() } {
            return true;
        }
        let icon_info = unsafe { &*(*icon).iconInfo };
        if icon_info.bitsColor.is_null() {
            return true;
        }
        let w = icon_info.width;
        let h = icon_info.height;

        // Sanity check for icon size to prevent DoS
        if w > 256 || h > 256 {
            log::warn!("RAIL: Ignoring too large icon: {}x{}", w, h);
            return true;
        }

        let cache_id = icon_info.cacheId;
        let cache_entry = icon_info.cacheEntry;

        let mut rgba = Vec::with_capacity((w as usize) * (h as usize) * 4);

        match icon_info.bpp {
            32 => {
                let len = (w as usize) * (h as usize) * 4;
                if (icon_info.cbBitsColor as usize) < len {
                    log::error!("RAIL: Icon buffer too small for 32-bpp {}x{}", w, h);
                    return true;
                }
                let src = unsafe { std::slice::from_raw_parts(icon_info.bitsColor, len) };
                for chunk in src.chunks_exact(4) {
                    rgba.push(chunk[2]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[0]); // B
                    rgba.push(chunk[3]); // A
                }
            }
            24 => {
                let len = (w as usize) * (h as usize) * 3;
                if (icon_info.cbBitsColor as usize) < len {
                    log::error!("RAIL: Icon buffer too small for 24-bpp {}x{}", w, h);
                    return true;
                }
                let src = unsafe { std::slice::from_raw_parts(icon_info.bitsColor, len) };
                for chunk in src.chunks_exact(3) {
                    rgba.push(chunk[2]); // R
                    rgba.push(chunk[1]); // G
                    rgba.push(chunk[0]); // B
                    rgba.push(255); // A
                }
            }
            16 => {
                let len = (w as usize) * (h as usize) * 2;
                if (icon_info.cbBitsColor as usize) < len {
                    log::error!("RAIL: Icon buffer too small for 16-bpp {}x{}", w, h);
                    return true;
                }
                let src = unsafe {
                    std::slice::from_raw_parts(
                        icon_info.bitsColor as *const u16,
                        (w as usize) * (h as usize),
                    )
                };
                for &pixel in src {
                    let r = ((pixel >> 11) & 0x1F) as u8;
                    let g = ((pixel >> 5) & 0x3F) as u8;
                    let b = (pixel & 0x1F) as u8;
                    rgba.push((r << 3) | (r >> 2));
                    rgba.push((g << 2) | (g >> 4));
                    rgba.push((b << 3) | (b >> 2));
                    rgba.push(255);
                }
            }
            _ => {
                log::warn!("RAIL: Unsupported icon bpp: {}", icon_info.bpp);
                return true;
            }
        }

        // Apply 1-bpp AND transparency mask if present
        if !icon_info.bitsMask.is_null() {
            let mask_stride = (w.div_ceil(32) * 4) as usize;
            let mask_len = mask_stride * h as usize;
            if (icon_info.cbBitsMask as usize) >= mask_len {
                let mask = unsafe { std::slice::from_raw_parts(icon_info.bitsMask, mask_len) };
                for y in 0..h as usize {
                    for x in 0..w as usize {
                        let byte_idx = y * mask_stride + (x / 8);
                        let bit_idx = 7 - (x % 8);
                        let is_transparent = (mask[byte_idx] & (1 << bit_idx)) != 0;
                        if is_transparent {
                            let rgba_idx = (y * w as usize + x) * 4;
                            if rgba_idx + 3 < rgba.len() {
                                rgba[rgba_idx + 3] = 0;
                            }
                        }
                    }
                }
            }
        }

        // Save in cache
        if let Ok(mut cache) = self.icon_cache.write() {
            cache.insert((cache_id, cache_entry), (rgba.clone(), w, h));
        }

        if let Some(tx) = &self.update_tx {
            let _ = tx.send(RdpMessage::WindowIcon {
                window_id,
                rgba,
                width: w,
                height: h,
            });
        }
        true
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn on_window_cached_icon(
        &self,
        order_info: *const WINDOW_ORDER_INFO,
        cached: *const WINDOW_CACHED_ICON_ORDER,
    ) -> bool {
        let is_individual = self.config.settings.rail.as_ref().map(|r| r.behavior)
            == Some(crate::settings::RailBehavior::IndividualWindows);
        if !is_individual {
            return true;
        }
        if order_info.is_null() || cached.is_null() {
            return true;
        }

        let window_id = unsafe { (*order_info).windowId };
        let cache_id = unsafe { (*cached).cachedIcon.cacheId };
        let cache_entry = unsafe { (*cached).cachedIcon.cacheEntry };

        if let Ok(cache) = self.icon_cache.read()
            && let Some((rgba, w, h)) = cache.get(&(cache_id, cache_entry))
            && let Some(tx) = &self.update_tx
        {
            let _ = tx.send(RdpMessage::WindowIcon {
                window_id,
                rgba: rgba.clone(),
                width: *w,
                height: *h,
            });
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_screen() {
        assert!(!is_offscreen_pos(100, 200));
        assert!(!is_offscreen_pos(-500, -500));
    }

    #[test]
    fn boundary_not_offscreen() {
        assert!(!is_offscreen_pos(OFFSCREEN_THRESHOLD, 0));
        assert!(!is_offscreen_pos(0, OFFSCREEN_THRESHOLD));
        assert!(!is_offscreen_pos(OFFSCREEN_THRESHOLD, OFFSCREEN_THRESHOLD));
    }

    #[test]
    fn offscreen_below_threshold() {
        assert!(is_offscreen_pos(OFFSCREEN_THRESHOLD - 1, 0));
        assert!(is_offscreen_pos(0, OFFSCREEN_THRESHOLD - 1));
        assert!(is_offscreen_pos(-2000, 0));
        assert!(is_offscreen_pos(0, -2000));
    }

    #[test]
    fn minimized_is_offscreen() {
        assert!(is_offscreen_pos(-32000, -32000));
    }
}
