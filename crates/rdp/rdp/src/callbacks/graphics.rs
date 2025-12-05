use freerdp_sys::{
    rdpPointer
};

use shared::log::debug;

pub trait GraphicsCallbacks {
    /// # Safety
    /// This function is unsafe because it dereferences a raw pointer to rdpPointer.
    unsafe fn on_pointer_new(&self, _pointer: *mut rdpPointer) -> bool {
        debug!("Pointer New callback not implemented");
        true
    }

    /// # Safety
    /// This function is unsafe because it dereferences a raw pointer to rdpPointer.
    unsafe fn on_pointer_free(&self, _pointer: *mut rdpPointer) {
        debug!("Pointer Free callback not implemented");
    }

    /// # Safety
    /// This function is unsafe because it dereferences a raw pointer to rdpPointer.
    unsafe fn on_pointer_set(&self, _pointer: *mut rdpPointer) -> bool {
        debug!("Pointer Set callback not implemented");
        true
    }

    fn on_pointer_set_null(&self) -> bool {
        debug!("Pointer SetNull callback not implemented");
        true
    }

    fn on_pointer_set_default(&self) -> bool {
        debug!("Pointer SetDefault callback not implemented");
        true
    }

    fn on_pointer_position(&self, _x: u32, _y: u32) -> bool {
        debug!("Pointer Position callback not implemented");
        true
    }
}