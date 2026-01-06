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
use freerdp_sys::*;
use shared::log;
use std::ffi::CStr;
use std::os::raw::c_char;

extern "C" fn my_message_cb(msg: *const wLogMessage) -> BOOL {
    if msg.is_null() {
        return 0;
    }

    let text_ptr = unsafe { (*msg).TextString };
    let text = if !text_ptr.is_null() {
        unsafe { CStr::from_ptr(text_ptr as *const c_char).to_string_lossy() }
    } else {
        std::borrow::Cow::Borrowed("")
    };

    unsafe {
        match (*msg).Level {
            WLOG_FATAL => log::error!(target: "freerdp", "FATAL: {}", text),
            WLOG_ERROR => log::error!(target: "freerdp", "{}", text),
            WLOG_WARN => log::warn!(target: "freerdp", "{}", text),
            WLOG_INFO => log::info!(target: "freerdp", "{}", text),
            WLOG_DEBUG => log::debug!(target: "freerdp", "{}", text),
            WLOG_TRACE => log::trace!(target: "freerdp", "{}", text),
            _ => log::info!(target: "freerdp", "{}", text),
        }
    };

    1 // TRUE
}

// Dumps settings uwing freerdp_settings_dump
#[allow(dead_code)]
/// # Safety
/// This function is unsafe because it dereferences raw pointers.
pub unsafe fn dump_freerdp_settings(settings: *mut rdpSettings) {
    unsafe {
        let log = WLog_GetRoot();
        if !settings.is_null() {
            freerdp_settings_dump(log, WLOG_DEBUG, settings);
        }
    }
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
pub enum WLogLevel {
    Fatal = WLOG_FATAL as isize,
    Error = WLOG_ERROR as isize,
    Warn = WLOG_WARN as isize,
    Info = WLOG_INFO as isize,
    Debug = WLOG_DEBUG as isize,
    Trace = WLOG_TRACE as isize,
}

#[allow(dead_code)]
pub fn set_wlog_level(tag: Option<&str>, level: WLogLevel) {
    unsafe {
        let log = match tag {
            Some(t) => {
                let c_tag = std::ffi::CString::new(t).unwrap();
                WLog_Get(c_tag.as_ptr())
            }
            None => WLog_GetRoot(),
        };
        if log.is_null() {
            log::error!("WLog_Get returned null for tag {:?}", tag);
            return;
        }
        WLog_SetLogLevel(log, level as u32);
    }
}

#[allow(clippy::manual_c_str_literals)]
pub fn setup_freerdp_logger(level: WLogLevel) {
    unsafe {
        let callbacks = wLogCallbacks {
            data: None,
            image: None,
            message: Some(my_message_cb),
            package: None,
        };

        let root = WLog_GetRoot();
        WLog_SetLogAppenderType(root, WLOG_APPENDER_CALLBACK);
        let appender = WLog_GetLogAppender(root);

        WLog_ConfigureAppender(
            appender,
            b"callbacks\0".as_ptr() as *const ::std::os::raw::c_char,
            &callbacks as *const _ as *mut _,
        );

        set_wlog_level(None, level);
        set_wlog_level(Some("com.freerdp.utils.ringbuffer"), WLogLevel::Info);
        set_wlog_level(Some("com.freerdp.primitives"), WLogLevel::Trace);
    }
}
