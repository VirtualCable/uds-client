use freerdp_sys::{
    rdpPointer
};

use shared::log;

pub trait GraphicsCallbacks {
    fn on_pointer_new(&self, _pointer: *mut rdpPointer) -> bool {
        log::debug!("🖱️ Pointer New callback not implemented");
        true
    }

    fn on_pointer_free(&self, _pointer: *mut rdpPointer) {
        log::debug!("🖱️ Pointer Free callback not implemented");
    }

    fn on_pointer_set(&self, _pointer: *mut rdpPointer) -> bool {
        log::debug!("🖱️ Pointer Set callback not implemented");
        true
    }

    fn on_pointer_set_null(&self) -> bool {
        log::debug!("🖱️ Pointer SetNull callback not implemented");
        true
    }

    fn on_pointer_set_default(&self) -> bool {
        log::debug!("🖱️ Pointer SetDefault callback not implemented");
        true
    }

    fn on_pointer_position(&self, _x: u32, _y: u32) -> bool {
        log::debug!("🖱️ Pointer Position callback not implemented");
        true
    }
}