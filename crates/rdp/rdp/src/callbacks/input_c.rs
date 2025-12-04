use freerdp_sys::{BOOL, INT16, UINT8, UINT16, UINT32, rdpContext, rdpInput};

use shared::log::debug;

use super::{super::context::OwnerFromCtx, input::InputCallbacks};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Callbacks {
    Keyboard,
    UnicodeKeyboard,
    Mouse,
    ExtendedMouse,
    FocusIn,
    KeyboardPause,
    RelMouse,
    Synchronize,
    QoE,
}

impl Callbacks {
    #[allow(dead_code)]
    pub fn all() -> Vec<Callbacks> {
        vec![
            Callbacks::Keyboard,
            Callbacks::UnicodeKeyboard,
            Callbacks::Mouse,
            Callbacks::ExtendedMouse,
            Callbacks::FocusIn,
            Callbacks::KeyboardPause,
            Callbacks::RelMouse,
            Callbacks::Synchronize,
            Callbacks::QoE,
        ]
    }
}

/// # Safety
/// This function is unsafe because it dereferences raw pointers to set callback functions.
pub unsafe fn set_callbacks(context: *mut rdpContext, overrides: &[Callbacks]) {
    unsafe {
        let input = (*context).input;
        if input.is_null() {
            debug!(" ⁉️ **** Input not initialized, cannot override callbacks.");
            return;
        }
        for override_cb in overrides {
            match override_cb {
                Callbacks::Keyboard => {
                    (*input).KeyboardEvent = Some(keyboard_event);
                }
                Callbacks::UnicodeKeyboard => {
                    (*input).UnicodeKeyboardEvent = Some(unicode_keyboard_event);
                }
                Callbacks::Mouse => {
                    (*input).MouseEvent = Some(mouse_event);
                }
                Callbacks::ExtendedMouse => {
                    (*input).ExtendedMouseEvent = Some(extended_mouse_event);
                }
                Callbacks::FocusIn => {
                    (*input).FocusInEvent = Some(focus_in_event);
                }
                Callbacks::KeyboardPause => {
                    (*input).KeyboardPauseEvent = Some(keyboard_pause_event);
                }
                Callbacks::RelMouse => {
                    (*input).RelMouseEvent = Some(rel_mouse_event);
                }
                Callbacks::Synchronize => {
                    (*input).SynchronizeEvent = Some(synchronize_event);
                }
                Callbacks::QoE => {
                    (*input).QoEEvent = Some(qoe_event);
                }
            }
        }
    }
}

pub extern "C" fn synchronize_event(input: *mut rdpInput, flags: UINT32) -> BOOL {
    if let Some(owner) = input.owner() {
        owner.on_synchronize_event(flags).into()
    } else {
        true.into()
    }
}

pub extern "C" fn keyboard_event(input: *mut rdpInput, flags: UINT16, code: UINT8) -> BOOL {
    if let Some(owner) = input.owner() {
        owner.on_keyboard_event(flags, code).into()
    } else {
        true.into()
    }
}

pub extern "C" fn unicode_keyboard_event(
    input: *mut rdpInput,
    flags: UINT16,
    code: UINT16,
) -> BOOL {
    if let Some(owner) = input.owner() {
        owner.on_unicode_keyboard_event(flags, code).into()
    } else {
        true.into()
    }
}

pub extern "C" fn mouse_event(input: *mut rdpInput, flags: UINT16, x: UINT16, y: UINT16) -> BOOL {
    if let Some(owner) = input.owner() {
        owner.on_mouse_event(flags, x, y).into()
    } else {
        true.into()
    }
}

pub extern "C" fn extended_mouse_event(
    input: *mut rdpInput,
    flags: UINT16,
    x: UINT16,
    y: UINT16,
) -> BOOL {
    if let Some(owner) = input.owner() {
        owner.on_extended_mouse_event(flags, x, y).into()
    } else {
        true.into()
    }
}

pub extern "C" fn focus_in_event(input: *mut rdpInput, toggle_states: UINT16) -> BOOL {
    if let Some(owner) = input.owner() {
        owner.on_focus_in_event(toggle_states).into()
    } else {
        true.into()
    }
}

pub extern "C" fn keyboard_pause_event(input: *mut rdpInput) -> BOOL {
    if let Some(owner) = input.owner() {
        owner.on_keyboard_pause_event().into()
    } else {
        true.into()
    }
}

pub extern "C" fn rel_mouse_event(input: *mut rdpInput, flags: UINT16, x: INT16, y: INT16) -> BOOL {
    if let Some(owner) = input.owner() {
        owner.on_rel_mouse_event(flags, x, y).into()
    } else {
        true.into()
    }
}

pub extern "C" fn qoe_event(input: *mut rdpInput, flags: UINT32) -> BOOL {
    if let Some(owner) = input.owner() {
        owner.on_qoe_event(flags).into()
    } else {
        true.into()
    }
}
