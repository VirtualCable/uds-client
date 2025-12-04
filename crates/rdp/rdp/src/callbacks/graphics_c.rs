use freerdp_sys::{BOOL, rdpContext, rdpPointer};

use shared::log::debug;

use super::{super::context::OwnerFromCtx, graphics::GraphicsCallbacks};

/// # Safety
/// This function is unsafe because it dereferences raw pointers to set callback functions.
pub unsafe fn set_callbacks(context: *mut rdpContext) {
    unsafe {
        let graphics = (*context).graphics;
        let pointer_proto = (*graphics).Pointer_Prototype;
        if graphics.is_null() || pointer_proto.is_null() {
            debug!(" **** Pointer not initialized, cannot override callbacks.");
            return;
        }
        // Clear pointer_proto to avoid dangling pointers
        std::ptr::write_bytes(pointer_proto, 0, 1);
        (*pointer_proto).New = Some(pointer_new);
        (*pointer_proto).Free = Some(pointer_free);
        (*pointer_proto).Set = Some(pointer_set);
        (*pointer_proto).SetNull = Some(pointer_set_null);
        (*pointer_proto).SetDefault = Some(pointer_set_default);
        (*pointer_proto).SetPosition = Some(pointer_position);
    }
}

pub extern "C" fn pointer_new(context: *mut rdpContext, pointer_new: *mut rdpPointer) -> BOOL {
    debug!(" 🌚 **** PointerNew called.");
    if let Some(rdp) = context.owner() {
        rdp.on_pointer_new(pointer_new).into()
    } else {
        true.into()
    }
}

pub extern "C" fn pointer_free(context: *mut rdpContext, pointer: *mut rdpPointer) {
    debug!(" 🌚 **** PointerFree called.");
    if let Some(rdp) = context.owner() {
        rdp.on_pointer_free(pointer);
    }
}

pub extern "C" fn pointer_set(context: *mut rdpContext, pointer_set: *mut rdpPointer) -> BOOL {
    debug!(" 🌚 **** PointerSet called.");
    if let Some(rdp) = context.owner() {
        rdp.on_pointer_set(pointer_set).into()
    } else {
        true.into()
    }
}
pub extern "C" fn pointer_set_null(context: *mut rdpContext) -> BOOL {
    debug!(" 🌚 **** PointerSetNull called.");
    if let Some(rdp) = context.owner() {
        rdp.on_pointer_set_null().into()
    } else {
        true.into()
    }
}

pub extern "C" fn pointer_set_default(context: *mut rdpContext) -> BOOL {
    debug!(" 🌚 **** PointerSetDefault called.");
    if let Some(rdp) = context.owner() {
        rdp.on_pointer_set_default().into()
    } else {
        true.into()
    }
}

pub extern "C" fn pointer_position(context: *mut rdpContext, x: u32, y: u32) -> BOOL {
    debug!(" 🌚 **** PointerPosition called.");
    if let Some(rdp) = context.owner() {
        rdp.on_pointer_position(x, y).into()
    } else {
        true.into()
    }
}
