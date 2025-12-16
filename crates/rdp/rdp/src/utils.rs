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
use std::ops::Deref;

use freerdp_sys::{GDI_RGN, HANDLE};
use shared::log;

use super::geom::Rect;

pub fn pixel_format(bpp: u8, pixel_type: u8, a: u8, r: u8, g: u8, b: u8) -> u32 {
    ((bpp as u32) << 24)
        | ((pixel_type as u32) << 16)
        | ((a as u32) << 12)
        | ((r as u32) << 8)
        | ((g as u32) << 4)
        | (b as u32)
}

pub trait ToStringLossy {
    fn to_string_lossy(&self) -> String;
}

impl ToStringLossy for *const i8 {
    fn to_string_lossy(&self) -> String {
        if self.is_null() {
            return String::new();
        }
        unsafe {
            std::ffi::CStr::from_ptr(*self)
                .to_string_lossy()
                .into_owned()
        }
    }
}

pub fn normalize_rects(rects_raw: &[GDI_RGN], width: u32, height: u32) -> Option<Vec<Rect>> {
    let width = width as i32;
    let height = height as i32;
    rects_raw
        .iter()
        .filter_map(|r| {
            if r.x <= width
                && r.y < height
                && r.x >= 0
                && r.y >= 0
                && r.w <= width
                && r.w > 0
                && r.h <= height
                && r.h > 0
            {
                Some(Rect {
                    x: r.x as u32,
                    y: r.y as u32,
                    w: r.w as u32,
                    h: r.h as u32,
                })
            } else {
                log::debug!("Skipping invalid rect: {:?}", r);
                None
            }
        })
        .collect::<Vec<_>>()
        .into()

    // for rect in rects_raw {
    //     // If any rect is invalid, skip it
    //     if rect.x > width
    //         || rect.x < 0
    //         || rect.y > height
    //         || rect.y < 0
    //         || rect.w > width
    //         || rect.w < 0
    //         || rect.h > height
    //         || rect.h < 0
    //     {
    //         log::debug!("Skipping invalid rect: {:?}", rect);
    //         log::debug!("All rects: {:?}", rects_raw);
    //         return Some(vec![Rect{ x: 0, y: 0, w: width as u32, h: height as u32 }])
    //     }

    //     #[allow(clippy::unnecessary_cast)] // Maybe on other platforms is not an i32...
    //     rects.push(rect.into());
    // }
    // if rects.is_empty() { None } else { Some(rects) }
}

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct SafePtr<T> {
    ptr: std::ptr::NonNull<std::os::raw::c_void>,
    _marker: std::marker::PhantomData<T>,
}

unsafe impl<T> Send for SafePtr<T> {}
unsafe impl<T> Sync for SafePtr<T> {}

impl<T> SafePtr<T> {
    pub fn new(ptr: *mut T) -> Option<Self> {
        std::ptr::NonNull::new(ptr as *mut std::os::raw::c_void).map(|nn| Self {
            ptr: nn,
            _marker: std::marker::PhantomData,
        })
    }

    pub fn as_ptr(&self) -> *const T {
        self.ptr.as_ptr() as *const T
    }

    pub fn as_mut_ptr(&self) -> *mut T {
        self.ptr.as_ptr() as *mut T
    }
}

impl<T> Deref for SafePtr<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.as_ptr()) }
    }
}

pub type SafeHandle = SafePtr<std::os::raw::c_void>;

impl SafeHandle {
    pub fn as_handle(&self) -> HANDLE {
        self.ptr.as_ptr() as HANDLE
    }
}
