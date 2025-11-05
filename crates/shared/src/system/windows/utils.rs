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
