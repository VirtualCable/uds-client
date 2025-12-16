// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use anyhow::Result;

use freerdp_sys::*;

use shared::log::debug;

use super::{Rdp, addins::addin};
use crate::callbacks::entrypoint_c;

#[derive(Debug)]
#[repr(C)]
pub struct RdpContext {
    pub common: rdpClientContext,
    pub owner: *mut Rdp,
}

impl RdpContext {
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
            // Override the addin provider to our custom one (in fact, "hook" the existing one)
            addin::register_channel_addin_loader();

            let ctx = ctx_ptr as *mut RdpContext;
            (*ctx).owner = owner as *mut Rdp;

            Ok(ctx)
        }
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

pub fn owner_from_ctx<'a>(ctx: *mut rdpContext) -> Option<&'a mut crate::Rdp> {
    unsafe {
        if ctx.is_null() {
            return None;
        }
        let ctx = ctx as *mut RdpContext;
        (*ctx).owner.as_mut()
    }
}
