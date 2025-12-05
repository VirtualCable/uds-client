use freerdp_sys::*;

use shared::log;

use crate::callbacks::graphics;

use super::Rdp;

impl graphics::GraphicsCallbacks for Rdp {
    fn on_pointer_set(&self, _pointer: *mut rdpPointer) -> bool {
        log::debug!(" **** Custom Pointer Set called.");
        // Here you can handle the custom pointer setting logic
        true
    }
}
