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
use freerdp_sys::{
    BOOL, POINTER_CACHED_UPDATE, POINTER_COLOR_UPDATE, POINTER_LARGE_UPDATE, POINTER_NEW_UPDATE,
    POINTER_POSITION_UPDATE, POINTER_SYSTEM_UPDATE, rdpContext,
};

use shared::log;

use super::super::context::OwnerFromCtx;
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
