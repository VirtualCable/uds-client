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
use std::sync::{Arc, RwLock};

mod callbacks_c;
pub(super) mod native;
mod traits;

use crate::utils;

pub use callbacks_c::register_cliprdr_callbacks;
pub use traits::ClipboardHandler;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum RdpClipboardFormat {
    TextUnicode, // Unicode Text, UTF-16 LE
    TextAscii,   // ASCII Text, ANSI
    TextOem,     // OEM Text
    Image,
}

impl RdpClipboardFormat {
    pub fn from_format(format: &freerdp_sys::CLIPRDR_FORMAT) -> Option<Self> {
        Self::from_format_id(format.formatId)
    }

    pub fn from_format_id(format_id: u32) -> Option<Self> {
        match format_id {
            freerdp_sys::CF_UNICODETEXT => Some(RdpClipboardFormat::TextUnicode),
            freerdp_sys::CF_TEXT => Some(RdpClipboardFormat::TextAscii),
            freerdp_sys::CF_OEMTEXT => Some(RdpClipboardFormat::TextOem),
            freerdp_sys::CF_DIB => Some(RdpClipboardFormat::Image),
            _ => None,
        }
    }

    pub fn to_format_id(&self) -> freerdp_sys::UINT32 {
        match self {
            RdpClipboardFormat::TextUnicode => freerdp_sys::CF_UNICODETEXT,
            RdpClipboardFormat::TextAscii => freerdp_sys::CF_TEXT,
            RdpClipboardFormat::TextOem => freerdp_sys::CF_OEMTEXT,
            RdpClipboardFormat::Image => freerdp_sys::CF_DIB,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RdpClipboard {
    ptr: Option<utils::SafePtr<freerdp_sys::CliprdrClientContext>>,

    pub caps_flags: u32,
    pub remote_formats: Vec<RdpClipboardFormat>, // Currently, we only support text format, but can be extended
    pub text: Arc<RwLock<String>>,               // Cached text from local clipboard
}

impl RdpClipboard {
    pub fn new(ptr: *mut freerdp_sys::CliprdrClientContext) -> Self {
        Self {
            ptr: utils::SafePtr::new(ptr),
            caps_flags: 0, // Currently we dont use the caps_flags from remote server, we assume server supports text formats :)
            remote_formats: Vec::new(),
            text: Arc::new(RwLock::new(String::new())),
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

    pub fn send_client_capabilities(&self, flags: u32) -> u32 {
        let general_capability_set = freerdp_sys::CLIPRDR_GENERAL_CAPABILITY_SET {
            capabilitySetType: freerdp_sys::CB_CAPSTYPE_GENERAL as u16,
            capabilitySetLength: std::mem::size_of::<freerdp_sys::CLIPRDR_GENERAL_CAPABILITY_SET>()
                as u16,
            version: freerdp_sys::CB_CAPS_VERSION_2,
            generalFlags: flags,
        };

        let capabilities = freerdp_sys::CLIPRDR_CAPABILITIES {
            common: freerdp_sys::CLIPRDR_HEADER {
                msgType: 0,
                msgFlags: 0,
                dataLen: 0,
            },

            cCapabilitiesSets: 1,
            capabilitySets: &general_capability_set as *const _ as *mut _,
        };
        self.client_capabilities(&capabilities)
    }

    // Internal helper to send client format list
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

    // Helper to send client format list but list is a slice of ClipboardFormat
    // When data is locally avaiable on clipboard, use this to send supported formats on the clipboard to server
    // Example. On a text editor press CTRL+C, "this" app receives the notificiation, obtains what formats are available on clipboard,
    // and calls this method to inform server about available formats. So the server also knows what formats are available to request data from.
    pub fn send_client_format_list(&self, formats: &[RdpClipboardFormat]) -> u32 {
        let mut cliprdr_formats: Vec<freerdp_sys::CLIPRDR_FORMAT> = Vec::new();

        for format in formats {
            cliprdr_formats.push(freerdp_sys::CLIPRDR_FORMAT {
                formatId: format.to_format_id(),
                formatName: std::ptr::null_mut(),
            });
        }

        let format_list = freerdp_sys::CLIPRDR_FORMAT_LIST {
            common: freerdp_sys::CLIPRDR_HEADER {
                msgType: 0,
                msgFlags: 0,
                dataLen: 0,
            },
            numFormats: cliprdr_formats.len() as u32,
            formats: cliprdr_formats.as_ptr() as *mut _,
        };

        self.client_format_list(&format_list)
    }

    // Helper to set send format list response to server and store text
    pub fn send_text_is_available(&self, text: &str) -> u32 {
        // Store locally
        {
            let mut local_text = self.text.write().unwrap();
            *local_text = text.to_string();
        }
        // Send format list to server
        self.send_client_format_list(&[RdpClipboardFormat::TextUnicode])
    }

    pub fn get_local_text(&self) -> String {
        let local_text = self.text.read().unwrap();
        local_text.clone()
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

    pub fn send_client_format_data_response(
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

    // Used to request clipboard data in specific format from server
    pub fn client_format_data_request(
        &self,
        format_data_request: &freerdp_sys::CLIPRDR_FORMAT_DATA_REQUEST,
    ) -> u32 {
        if let Some(ptr) = &self.ptr
            && let Some(func) = ptr.ClientFormatDataRequest
        {
            unsafe {
                func(
                    ptr.as_mut_ptr(),
                    format_data_request as *const freerdp_sys::CLIPRDR_FORMAT_DATA_REQUEST,
                )
            }
        } else {
            freerdp_sys::CHANNEL_RC_OK
        }
    }
}
