use freerdp_sys::{BOOL, freerdp, rdpContext};

use super::{super::context::OwnerFromCtx, entrypoint::EntrypointCallbacks};
use crate::context::RdpContext;
use shared::log;

pub extern "C" fn client_global_init() -> BOOL {
    // We could do the WSA initialization here if needed
    log::debug!(" **** RDP client global init called");
    super::super::init::initialize();
    true.into()
}

pub extern "C" fn client_global_uninit() {
    // Currently, we do not need any special handling here.
    log::debug!(" **** RDP client global uninit called");
    super::super::init::uninitialize();
}

pub extern "C" fn client_new(instance: *mut freerdp, context: *mut rdpContext) -> BOOL {
    // Currently, we do not need any special handling here.
    // Note, here we do not have the owner initialized, just for future reference.
    let ctx = context as *mut RdpContext;
    log::debug!(
        " **** RDP client new instance created: {:?} -- {:?} ({:?})",
        instance,
        ctx,
        unsafe { (*ctx).owner }
    );
    true.into()
}

pub extern "C" fn client_free(_instance: *mut freerdp, _context: *mut rdpContext) {
    // Currently, we do not need any special handling here.
}

pub extern "C" fn client_start(context: *mut rdpContext) -> ::std::os::raw::c_int {
    log::debug!(
        " **** RDP client start called with context: {:?}",
        context
    );
    if let Some(owner) = context.owner() {
        owner.client_start().into()
    } else {
        true.into()
    }
}

pub extern "C" fn client_stop(context: *mut rdpContext) -> ::std::os::raw::c_int {
    if let Some(owner) = context.owner() {
        owner.client_stop().into()
    } else {
        true.into()
    }
}
