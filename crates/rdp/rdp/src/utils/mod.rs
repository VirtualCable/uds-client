// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

pub mod graphics;
pub mod log;
pub mod ptr;
pub mod trigger;

pub use graphics::{normalize_rects, pixel_format};
pub use ptr::{SafeHandle, SafePtr, ToStringLossy};

use zeroize::Zeroize;

pub fn zeroize_cstring(s: &mut std::ffi::CString) {
    let bytes = unsafe {
        let ptr = s.as_ptr() as *mut u8;
        let len = s.as_bytes_with_nul().len();
        std::slice::from_raw_parts_mut(ptr, len)
    };
    bytes.zeroize();
}
