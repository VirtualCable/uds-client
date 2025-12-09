mod callbacks_c;
mod traits;

use crate::utils;

pub use callbacks_c::register_cliprdr_callbacks;
pub use traits::ClipboardHandler;

const HTML_FORMAT_NAME: &str = "HTML Format";
const FILE_GROUP_DESCRIPTOR_W_NAME: &str = "FileGroupDescriptorW";

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ClipboardFormat {
    Text,
    Html,
    Image,
    File,
}

impl ClipboardFormat {
    pub fn from_format(format: &freerdp_sys::CLIPRDR_FORMAT) -> Option<Self> {
        unsafe {
            if !format.formatName.is_null() {
                let c_str = std::ffi::CStr::from_ptr(format.formatName);
                if let Ok(name) = c_str.to_str() {
                    match name {
                        HTML_FORMAT_NAME => {
                            return Some(ClipboardFormat::Html);
                        }
                        FILE_GROUP_DESCRIPTOR_W_NAME => {
                            return Some(ClipboardFormat::File);
                        }
                        _ => {}
                    }
                }
            }
        }
        Self::from_format_id(format.formatId)
    }

    pub fn from_format_id(format_id: u32) -> Option<Self> {
        match format_id {
            freerdp_sys::CF_UNICODETEXT | freerdp_sys::CF_OEMTEXT | freerdp_sys::CF_TEXT => {
                Some(ClipboardFormat::Text)
            }
            freerdp_sys::CF_DIB | freerdp_sys::CF_DIBV5 => {
                Some(ClipboardFormat::Image)
            }
            _ => {
                None
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Clipboard {
    ptr: Option<utils::SafePtr<freerdp_sys::CliprdrClientContext>>,

    pub caps_flags: u32,
    pub formats: Vec<ClipboardFormat>, // Currently, we only support text format, but can be extended
}

impl Clipboard {
    pub fn new(ptr: *mut freerdp_sys::CliprdrClientContext) -> Self {
        Self {
            ptr: utils::SafePtr::new(ptr),
            caps_flags: 0,
            formats: Vec::new(),
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

    pub fn send_format_list_response(&self, success: bool) -> u32 {
        if let Some(ptr) = &self.ptr
            && let Some(func) = ptr.ClientFormatListResponse
        {
            let response = freerdp_sys::CLIPRDR_FORMAT_LIST_RESPONSE {
                common: freerdp_sys::CLIPRDR_HEADER {
                    msgType: 0,
                    msgFlags: if success {
                        freerdp_sys::CB_RESPONSE_OK as u16
                    } else {
                        freerdp_sys::CB_RESPONSE_FAIL as u16
                    },
                    dataLen: 0,
                },
            };
            unsafe {
                func(
                    ptr.as_mut_ptr(),
                    &response as *const freerdp_sys::CLIPRDR_FORMAT_LIST_RESPONSE,
                )
            }
        } else {
            freerdp_sys::CHANNEL_RC_OK
        }
    }

    pub fn send_format_data_response(
        &self,
        format_data_response: &freerdp_sys::CLIPRDR_FORMAT_DATA_RESPONSE,
    ) -> u32 {
        if let Some(ptr) = &self.ptr
            && let Some(func) = ptr.ClientFormatDataResponse
        {
            unsafe {
                func(
                    ptr.as_mut_ptr(),
                    format_data_response as *const freerdp_sys::CLIPRDR_FORMAT_DATA_RESPONSE,
                )
            }
        } else {
            freerdp_sys::CHANNEL_RC_OK
        }
    }
}
