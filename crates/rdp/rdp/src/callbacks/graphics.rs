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