use freerdp_sys::{INT16, UINT8, UINT16, UINT32};

use shared::log;

pub trait InputCallbacks {
    fn on_synchronize_event(&mut self, flags: UINT32) -> bool {
        log::debug!("Synchronize event: flags={}", flags);
        true
    }

    fn on_keyboard_event(&mut self, flags: UINT16, code: UINT8) -> bool {
        log::debug!("Keyboard event: flags={}, code={}", flags, code);
        true
    }

    fn on_unicode_keyboard_event(&mut self, flags: UINT16, code: UINT16) -> bool {
        log::debug!("Unicode keyboard event: flags={}, code={}", flags, code);
        true
    }

    fn on_mouse_event(&mut self, flags: UINT16, x: UINT16, y: UINT16) -> bool {
        log::debug!("Mouse event: flags={}, x={}, y={}", flags, x, y);
        true
    }

    fn on_extended_mouse_event(&mut self, flags: UINT16, x: UINT16, y: UINT16) -> bool {
        log::debug!("Extended mouse event: flags={}, x={}, y={}", flags, x, y);
        true
    }

    fn on_focus_in_event(&mut self, toggle_states: UINT16) -> bool {
        log::debug!("Focus in event: toggle_states={}", toggle_states);
        true
    }

    fn on_keyboard_pause_event(&mut self) -> bool {
        log::debug!("Keyboard pause event");
        true
    }

    fn on_rel_mouse_event(&mut self, flags: UINT16, x: INT16, y: INT16) -> bool {
        log::debug!("Relative mouse event: flags={}, x={}, y={}", flags, x, y);
        true
    }

    fn on_qoe_event(&mut self, flags: UINT32) -> bool {
        log::debug!("QoE event: flags={}", flags);
        true
    }
}
