use freerdp_sys::{
    BOOL, POINTER_CACHED_UPDATE, POINTER_COLOR_UPDATE, POINTER_LARGE_UPDATE, POINTER_NEW_UPDATE,
    POINTER_POSITION_UPDATE, POINTER_SYSTEM_UPDATE, rdpContext,
};

use shared::log;

use super::super::connection::context::OwnerFromCtx;
use super::pointer_update::PointerCallbacks;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Callbacks {
    Position,
    System,
    Color,
    New,
    Cached,
    Large,
}

impl Callbacks {
    #[allow(dead_code)]
    pub fn all() -> Vec<Callbacks> {
        vec![
            Callbacks::Position,
            Callbacks::System,
            Callbacks::Color,
            Callbacks::New,
            Callbacks::Cached,
            Callbacks::Large,
        ]
    }
}

/// # Safety
/// This function is unsafe because it dereferences raw pointers to set callback functions.
pub unsafe fn set_callbacks(context: *mut rdpContext, overrides: &[Callbacks]) {
    log::debug!(" **** Setting Pointer Update Callbacks: {:?}", overrides);
    unsafe {
        let update = (*context).update;
        let pointer = (*update).pointer;
        if update.is_null() || pointer.is_null() {
            log::debug!(" **** Pointer not initialized, cannot override callbacks.");
            return;
        }
        for override_cb in overrides {
            match override_cb {
                Callbacks::Position => {
                    (*pointer).PointerPosition = Some(pointer_position);
                }
                Callbacks::System => {
                    (*pointer).PointerSystem = Some(pointer_system);
                }
                Callbacks::Color => {
                    (*pointer).PointerColor = Some(pointer_color);
                }
                Callbacks::New => {
                    (*pointer).PointerNew = Some(pointer_new);
                }
                Callbacks::Cached => {
                    (*pointer).PointerCached = Some(pointer_cached);
                }
                Callbacks::Large => {
                    (*pointer).PointerLarge = Some(pointer_large);
                }
            }
        }
    }
}

pub extern "C" fn pointer_position(
    context: *mut rdpContext,
    pointer_position: *const POINTER_POSITION_UPDATE,
) -> BOOL {
    log::debug!(" **** Pointer Position callback invoked: {:?}", pointer_position);
    if let Some(owner) = context.owner() {
        owner.on_pointer_position(pointer_position).into()
    } else {
        true.into()
    }
}

pub extern "C" fn pointer_system(
    context: *mut rdpContext,
    pointer_system: *const POINTER_SYSTEM_UPDATE,
) -> BOOL {
    log::debug!(" **** Pointer System callback invoked: {:?}", pointer_system);
    if let Some(owner) = context.owner() {
        owner.on_pointer_system(pointer_system).into()
    } else {
        true.into()
    }
}

pub extern "C" fn pointer_color(
    context: *mut rdpContext,
    pointer_color: *const POINTER_COLOR_UPDATE,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_pointer_color(pointer_color).into()
    } else {
        true.into()
    }
}

pub extern "C" fn pointer_new(
    context: *mut rdpContext,
    pointer_new: *const POINTER_NEW_UPDATE,
) -> BOOL {
    log::debug!(" **** Pointer New callback invoked: {:?}", pointer_new);
    if let Some(owner) = context.owner() {
        owner.on_pointer_new(pointer_new).into()
    } else {
        true.into()
    }
}

pub extern "C" fn pointer_cached(
    context: *mut rdpContext,
    pointer_cached: *const POINTER_CACHED_UPDATE,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_pointer_cached(pointer_cached).into()
    } else {
        true.into()
    }
}

pub extern "C" fn pointer_large(
    context: *mut rdpContext,
    pointer_large: *const POINTER_LARGE_UPDATE,
) -> BOOL {
    if let Some(owner) = context.owner() {
        owner.on_pointer_large(pointer_large).into()
    } else {
        true.into()
    }
}
