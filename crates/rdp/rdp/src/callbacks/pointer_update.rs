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
use freerdp_sys::{
    POINTER_CACHED_UPDATE, POINTER_COLOR_UPDATE, POINTER_LARGE_UPDATE, POINTER_NEW_UPDATE, POINTER_POSITION_UPDATE, POINTER_SYSTEM_UPDATE
};

use shared::log::debug;

pub trait PointerCallbacks {
    fn on_pointer_position(&self, pointer_position: *const POINTER_POSITION_UPDATE) -> bool {
        debug!("Pointer position event: pointer_position={:?}", pointer_position);
        true
    }

    fn on_pointer_system(&self, pointer_system: *const POINTER_SYSTEM_UPDATE) -> bool {
        debug!("Pointer system event: pointer_system={:?}", pointer_system);
        true
    }

    fn on_pointer_color(&self, pointer_color: *const POINTER_COLOR_UPDATE) -> bool {
        debug!("Pointer color event: pointer_color={:?}", pointer_color);
        true
    }

    fn on_pointer_new(&self, pointer_new: *const POINTER_NEW_UPDATE) -> bool {
        debug!("Pointer new event: pointer_new={:?}", pointer_new);
        true
    }

    fn on_pointer_cached(&self, pointer_cached: *const POINTER_CACHED_UPDATE) -> bool {
        debug!("Pointer cached event: pointer_cached={:?}", pointer_cached);
        true
    }

    fn on_pointer_large(&self, pointer_large: *const POINTER_LARGE_UPDATE) -> bool {
        debug!("Pointer large event: pointer_large={:?}", pointer_large);
        true
    }
}
