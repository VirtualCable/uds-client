use freerdp_sys::*;
use std::ffi::CStr;
use std::os::raw::c_char;

use shared::log;


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
/// Interoperability with C code.
/// Dumps FreeRDP settings to the logger.
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
            b"callbacks\0".as_ptr() as *const i8,
            &callbacks as *const _ as *mut _,
        );

        set_wlog_level(None, level);
        set_wlog_level(Some("com.freerdp.utils.ringbuffer"), WLogLevel::Info);
        set_wlog_level(Some("com.freerdp.primitives"), WLogLevel::Trace);
    }
}
