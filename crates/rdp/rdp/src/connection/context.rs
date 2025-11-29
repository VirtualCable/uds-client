use anyhow::Result;

use freerdp_sys::*;

use crate::callbacks::entrypoint_c;
use shared::log::debug;

use super::Rdp;

#[derive(Debug)]
#[repr(C)]
pub struct RdpContext {
    pub common: rdpClientContext,
    pub owner: *mut Rdp,
}

impl RdpContext {
    pub fn new() -> Self {
        RdpContext {
            common: unsafe { std::mem::zeroed() },
            owner: std::ptr::null_mut(),
        }
    }

    pub fn context(&self) -> &rdpContext {
        &self.common.context
    }

    pub fn create(owner: &mut Rdp) -> Result<*mut Self> {
        const FREERDP_CLIENT_INTERFACE_VERSION: u32 = 1;

        debug_assert!(
            std::mem::size_of::<rdpClientContext>() + std::mem::size_of::<*mut Rdp>()
                == std::mem::size_of::<RdpContext>(),
            "Size mismatch between rdpClientContext and RdpContext"
        );

        let entry_points = rdp_client_entry_points_v1 {
            Size: std::mem::size_of::<rdp_client_entry_points_v1>() as u32,
            Version: FREERDP_CLIENT_INTERFACE_VERSION,
            settings: std::ptr::null_mut(),
            GlobalInit: Some(entrypoint_c::client_global_init),
            GlobalUninit: Some(entrypoint_c::client_global_uninit),
            ContextSize: std::mem::size_of::<RdpContext>() as u32,
            ClientNew: Some(entrypoint_c::client_new),
            ClientFree: Some(entrypoint_c::client_free),
            ClientStart: Some(entrypoint_c::client_start),
            ClientStop: Some(entrypoint_c::client_stop),
        };

        unsafe {
            let ctx_ptr = freerdp_client_context_new(&entry_points);
            if ctx_ptr.is_null() {
                return Err(anyhow::anyhow!("Failed to create client context"));
            }

            let ctx = ctx_ptr as *mut RdpContext;
            (*ctx).owner = owner as *mut Rdp;

            Ok(ctx)
        }
    }
}

impl Default for RdpContext {
    fn default() -> Self {
        debug!(" *#*#*#+  Default RdpContext called");
        Self::new()
    }
}

impl Drop for RdpContext {
    fn drop(&mut self) {
        debug!("****** Dropping RdpContext, cleaning up resources... !!!!!!");
    }
}

pub trait OwnerFromCtx<'a> {
    fn owner(&self) -> Option<&'a mut Rdp>
    where
        for<'b> Self: Sized;
}

impl<'a> OwnerFromCtx<'a> for *mut rdpContext {
    fn owner(&self) -> Option<&'a mut Rdp> {
        owner_from_ctx(*self)
    }
}

impl<'a> OwnerFromCtx<'a> for *mut freerdp {
    fn owner(&self) -> Option<&'a mut Rdp> {
        owner_from_ctx(unsafe { (*(*self)).context })
    }
}

impl<'a> OwnerFromCtx<'a> for *mut rdpInput {
    fn owner(&self) -> Option<&'a mut Rdp> {
        unsafe {
            if self.is_null() {
                return None;
            }
            let ctx = (*(*self)).context;
            owner_from_ctx(ctx)
        }
    }
}

pub fn owner_from_ctx<'a>(ctx: *mut rdpContext) -> Option<&'a mut crate::connection::Rdp> {
    unsafe {
        if ctx.is_null() {
            return None;
        }
        let ctx = ctx as *mut RdpContext;
        (*ctx).owner.as_mut()
    }
}
