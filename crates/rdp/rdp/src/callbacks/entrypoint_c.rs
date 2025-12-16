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
