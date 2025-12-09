mod callbacks_c;
mod traits;

use crate::utils;

pub use callbacks_c::register_cliprdr_callbacks;
pub use traits::ClipboardHandler;

#[derive(Clone, Debug)]
pub struct Clipboard {
    ptr: Option<utils::SafePtr<freerdp_sys::CliprdrClientContext>>,
}

impl Clipboard {
    pub fn new(ptr: *mut freerdp_sys::CliprdrClientContext) -> Self {
        Self {
            ptr: utils::SafePtr::new(ptr),
        }
    }

    pub fn client_capabilities(&self, capabilities: &freerdp_sys::CLIPRDR_CAPABILITIES) -> u32 {
        if let Some(ptr) = &self.ptr
            && let Some(func) = ptr.ClientCapabilities
        {
            unsafe {
                func(
                    ptr.as_mut_ptr(),
                    capabilities as *const freerdp_sys::CLIPRDR_CAPABILITIES,
                )
            }
        } else {
            freerdp_sys::CHANNEL_RC_OK
        }
    }

    pub fn client_format_list(&self, format_list: &freerdp_sys::CLIPRDR_FORMAT_LIST) -> u32 {
        if let Some(ptr) = &self.ptr
            && let Some(func) = ptr.ClientFormatList
        {
            unsafe {
                func(
                    ptr.as_mut_ptr(),
                    format_list as *const freerdp_sys::CLIPRDR_FORMAT_LIST,
                )
            }
        } else {
            freerdp_sys::CHANNEL_RC_OK
        }
    }
}
