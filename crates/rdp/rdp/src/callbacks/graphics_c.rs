// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
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
use freerdp_sys::{BOOL, rdpContext, rdpPointer};

use super::{super::context::OwnerFromCtx, graphics::GraphicsCallbacks};

/// # Safety
/// This function is unsafe because it dereferences raw pointers to set callback functions.
pub unsafe fn set_callbacks(context: *mut rdpContext) {
    unsafe {
        let graphics = (*context).graphics;

        let pointer_proto = rdpPointer {
            size: std::mem::size_of::<rdpPointer>(),
            New: Some(pointer_new),
            Free: Some(pointer_free),
            Set: Some(pointer_set),
            SetNull: Some(pointer_set_null),
            SetDefault: Some(pointer_set_default),
            SetPosition: Some(pointer_position),
            paddingA: [0; 9],
            xPos: 0,
            yPos: 0,
            width: 0,
            height: 0,
            xorBpp: 0,
            lengthAndMask: 0,
            lengthXorMask: 0,
            xorMaskData: std::ptr::null_mut(),
            andMaskData: std::ptr::null_mut(),
            paddingB: [0; 7],
        };
        freerdp_sys::graphics_register_pointer(graphics, &pointer_proto);
    }
}

extern "C" fn pointer_new(context: *mut rdpContext, pointer_new: *mut rdpPointer) -> BOOL {
    if let Some(rdp) = context.owner() {
        unsafe { rdp.on_pointer_new(pointer_new).into() }
    } else {
        true.into()
    }
}

extern "C" fn pointer_free(context: *mut rdpContext, pointer: *mut rdpPointer) {
    if let Some(rdp) = context.owner() {
        unsafe { rdp.on_pointer_free(pointer); }
    }
}

extern "C" fn pointer_set(context: *mut rdpContext, pointer_set: *mut rdpPointer) -> BOOL {
    if let Some(rdp) = context.owner() {
        unsafe { rdp.on_pointer_set(pointer_set).into() }
    } else {
        true.into()
    }
}
extern "C" fn pointer_set_null(context: *mut rdpContext) -> BOOL {
    if let Some(rdp) = context.owner() {
        rdp.on_pointer_set_null().into()
    } else {
        true.into()
    }
}

extern "C" fn pointer_set_default(context: *mut rdpContext) -> BOOL {
    if let Some(rdp) = context.owner() {
        rdp.on_pointer_set_default().into()
    } else {
        true.into()
    }
}

extern "C" fn pointer_position(context: *mut rdpContext, x: u32, y: u32) -> BOOL {
    if let Some(rdp) = context.owner() {
        rdp.on_pointer_position(x, y).into()
    } else {
        true.into()
    }
}
