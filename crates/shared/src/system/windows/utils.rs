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
use std::sync::Arc;
use windows::Win32::Foundation::{HWND, LPARAM};

use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
};
use windows::core::BOOL;

pub fn check_if_any_visible_window(pids: &Vec<usize>) -> Option<HWND> {
    struct CallbackContext<'a> {
        pids: &'a Vec<usize>,
        found_hwnd: &'a Arc<std::sync::Mutex<Option<usize>>>,
    }

    // Shared storage for the found HWND
    let found_hwnd = Arc::new(std::sync::Mutex::new(None));
    let context = Box::new(CallbackContext {
        pids,
        found_hwnd: &found_hwnd,
    });

    // Convert the context to a raw pointer
    let ctx_ptr = Box::into_raw(context);
    let lparam = LPARAM(ctx_ptr as isize);

    // Enumerate windows
    unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx: &CallbackContext = unsafe { &*(lparam.0 as *const CallbackContext) };

        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };

        if ctx.pids.contains(&(pid as usize)) {
            let visible = unsafe { IsWindowVisible(hwnd) }.as_bool();

            if visible {
                // Store the found HWND and stop enumeration
                if let Ok(mut found) = ctx.found_hwnd.lock() {
                    *found = Some(hwnd.0 as usize);
                }
                return BOOL(0); // Stop enumeration as soon as we find a visible window
            }
        }

        BOOL(1) // Continue enumeration
    }

    unsafe {
        let _ = EnumWindows(Some(enum_windows_callback), lparam);
        // Free the context pointer
        drop(Box::from_raw(ctx_ptr));
    }

    // Extract the result, return none if not found
    if found_hwnd.lock().unwrap().is_none() {
        return None;
    }
    HWND(found_hwnd.lock().unwrap().unwrap_or(0) as *mut _).into()
}
