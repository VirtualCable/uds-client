use freerdp_sys::{BITMAP_UPDATE, RECTANGLE_16, rdpBounds};

use shared::log::debug;

pub trait UpdateCallbacks {
    fn on_begin_paint(&mut self) -> bool {
        true
    }

    fn on_end_paint(&mut self) -> bool {
        true
    }

    /// # Safety
    /// This callback function interoperates with C pointers.
    unsafe fn on_set_bounds(&mut self, bounds: *const rdpBounds) -> bool {
        let bounds = unsafe { *bounds };
        debug!(
            "Set bounds: left={}, top={}, right={}, bottom={}",
            bounds.left, bounds.top, bounds.right, bounds.bottom
        );
        true
    }

    fn on_synchronize(&mut self) -> bool {
        debug!("Synchronize");
        true
    }

    fn on_desktop_resize(&mut self) -> bool {
        debug!("Desktop resized");
        true
    }

    fn on_bitmap_update(&mut self, bitmap: *const BITMAP_UPDATE) -> bool {
        if bitmap.is_null() {
            debug!("Bitmap update called with null pointer");
            return false;
        }
        true
    }

    fn on_palette(&mut self, palette: *const freerdp_sys::PALETTE_UPDATE) -> bool {
        if palette.is_null() {
            debug!("Palette update called with null pointer");
            return false;
        }
        true
    }

    fn on_play_sound(&mut self, play_sound: *const freerdp_sys::PLAY_SOUND_UPDATE) -> bool {
        if play_sound.is_null() {
            debug!("Play sound update called with null pointer");
            return false;
        }
        true
    }

    fn on_set_keyboard_indicators(&mut self, led_flags: u16) -> bool {
        debug!("Set keyboard indicators: led_flags={}", led_flags);
        true
    }

    fn on_set_keyboard_ime_status(
        &mut self,
        ime_id: u16,
        ime_state: u32,
        ime_conv_mode: u32,
    ) -> bool {
        debug!(
            "Set keyboard IME status: ime_id={}, ime_state={}, ime_conv_mode={}",
            ime_id, ime_state, ime_conv_mode
        );
        true
    }

    fn on_refresh_rect(&mut self, count: u8, areas: *const RECTANGLE_16) -> bool {
        if areas.is_null() {
            debug!("Refresh rect called with null pointer");
            return false;
        }
        debug!("Refresh rect: count={}", count);
        true
    }

    fn on_suppress_output(&mut self, allow: u8, area: *const RECTANGLE_16) -> bool {
        if area.is_null() {
            debug!("Suppress output called with null pointer");
            return false;
        }
        debug!("Suppress output: allow={}", allow);
        true
    }

    fn on_remote_monitors(
        &mut self,
        count: u32,
        monitors: *const freerdp_sys::MONITOR_DEF,
    ) -> bool {
        if monitors.is_null() {
            debug!("Remote monitors called with null pointer");
            return false;
        }
        debug!("Remote monitors: count={}", count);
        true
    }

    fn on_surface_command(&mut self, s: *mut freerdp_sys::wStream) -> bool {
        if s.is_null() {
            debug!("Surface command called with null pointer");
            return false;
        }
        true
    }

    fn on_surface_bits(&mut self, surface_bits: *const freerdp_sys::SURFACE_BITS_COMMAND) -> bool {
        if surface_bits.is_null() {
            debug!("Surface bits called with null pointer");
            return false;
        }
        true
    }

    fn on_surface_frame_marker(
        &mut self,
        surface_frame_marker: *const freerdp_sys::SURFACE_FRAME_MARKER,
    ) -> bool {
        if surface_frame_marker.is_null() {
            debug!("Surface frame marker called with null pointer");
            return false;
        }
        true
    }

    fn on_surface_frame_bits(
        &mut self,
        cmd: *const freerdp_sys::SURFACE_BITS_COMMAND,
        first: bool,
        last: bool,
        frame_id: u32,
    ) -> bool {
        if cmd.is_null() {
            debug!("Surface frame bits called with null pointer");
            return false;
        }
        debug!(
            "Surface frame bits: first={}, last={}, frame_id={}",
            first, last, frame_id
        );
        true
    }

    fn on_surface_frame_acknowledge(&mut self, frame_id: u32) -> bool {
        debug!("Surface frame acknowledge: frame_id={}", frame_id);
        true
    }

    fn on_save_session_info(&mut self, type_: u32, data: *mut ::std::os::raw::c_void) -> bool {
        if data.is_null() {
            debug!("Save session info called with null pointer");
            return false;
        }
        debug!("Save session info: type={}", type_);
        true
    }

    fn on_server_status_info(&mut self, status: u32) -> bool {
        debug!("Server status info: status={}", status);
        true
    }
}
