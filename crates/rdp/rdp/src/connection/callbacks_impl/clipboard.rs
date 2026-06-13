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
use std::collections::HashSet;
use std::sync::Arc;

use crate::utils::log;

use super::Rdp;

use crate::channels::cliprdr::{ClipboardHandler, RdpClipboard, RdpClipboardFormat};
use crate::integrations::ClipboardCallback;

struct RdpClipboardCallback {
    rdp_clipboard: RdpClipboard,
}

impl ClipboardCallback for RdpClipboardCallback {
    fn send_text_is_available(&self, text: &str) {
        self.rdp_clipboard.send_text_is_available(text);
    }
}

impl ClipboardHandler for Rdp {
    fn on_monitor_ready(&mut self, monitor_ready: &freerdp_sys::CLIPRDR_MONITOR_READY) -> u32 {
        log::debug!(
            "Clipboard Monitor Ready event received in Rdp impl: {:?}",
            monitor_ready
        );
        if let Some(context) = self.channels.read().unwrap().cliprdr() {
            // Server expect capabilities + initial clipboard status available
            context.send_client_capabilities(freerdp_sys::CB_USE_LONG_FORMAT_NAMES);

            if let Some(ref clipboard_integration) = self.config.integrations.clipboard {
                clipboard_integration.start(Arc::new(RdpClipboardCallback {
                    rdp_clipboard: context.clone(),
                }));
                if let Ok(text) = clipboard_integration.get_text()
                    && !text.is_empty()
                {
                    context.send_text_is_available(&text);
                }
            }

            freerdp_sys::CHANNEL_RC_OK
        } else {
            log::error!("Clipboard context is null in Monitor Ready");
            freerdp_sys::CHANNEL_RC_TOO_MANY_CHANNELS
        }
    }

    fn on_receive_format_list_response(&mut self, success: bool) -> u32 {
        log::debug!(
            "Clipboard Receive Format List Response event received in Rdp impl: {:?}",
            success
        );
        if !success {
            log::warn!("Clipboard format list response: NOT OK");
        }

        freerdp_sys::CHANNEL_RC_OK
    }

    fn on_receive_server_capabilities(
        &mut self,
        capabilities: &freerdp_sys::CLIPRDR_CAPABILITIES,
    ) -> u32 {
        if let Some(mut context) = self.channels.write().unwrap().cliprdr() {
            log::debug!(
                "Clipboard Receive Server Capabilities event received in Rdp impl: {:?}",
                capabilities
            );
            let mut cap_set_ptr = capabilities.capabilitySets as *const u8;

            // Extract capability sets (a list with variable lengths)
            unsafe {
                for _ in 0..capabilities.cCapabilitiesSets {
                    let caps = cap_set_ptr as *const freerdp_sys::CLIPRDR_CAPABILITY_SET;
                    let cap_set = &*caps;
                    if cap_set.capabilitySetType == freerdp_sys::CB_CAPSTYPE_GENERAL as u16 {
                        let general_caps =
                            cap_set_ptr as *const freerdp_sys::CLIPRDR_GENERAL_CAPABILITY_SET;
                        let general_caps = &*general_caps;
                        log::debug!("General Capability Set: {:?}", general_caps);
                        context.caps_flags = general_caps.generalFlags;
                        break;
                    } else {
                        log::debug!("Other Capability Set Type: {:?}", cap_set);
                    }
                    cap_set_ptr = cap_set_ptr.add(cap_set.capabilitySetLength as usize);
                }
            }
        }

        freerdp_sys::CHANNEL_RC_OK
    }

    // This is called by server to inform client about available clipboard formats
    // on remote side. Our implementation should take into account only text formats for now.
    fn on_receive_server_format_list(
        &mut self,
        format_list: &freerdp_sys::CLIPRDR_FORMAT_LIST,
    ) -> u32 {
        if let Some(mut context) = self.channels.read().unwrap().cliprdr() {
            log::debug!(
                "Clipboard Receive Server Format List event received in Rdp impl: {:?}",
                format_list
            );
            context.remote_formats.clear();
            unsafe {
                let formats_ptr = format_list.formats as *const freerdp_sys::CLIPRDR_FORMAT;
                let mut supported: HashSet<RdpClipboardFormat> = HashSet::new();
                for i in 0..format_list.numFormats {
                    let format = &*formats_ptr.add(i as usize);
                    if let Some(clip_format) = RdpClipboardFormat::from_format(format) {
                        supported.insert(clip_format);
                    }
                }

                // Note: We are only interested right now on text formats
                context.remote_formats = supported.into_iter().collect(); // Convert HashSet to Vec

                log::debug!(
                    "Supported clipboard formats from remote: {:?}",
                    context.remote_formats
                );
            }
            context.send_format_list_response(!context.remote_formats.is_empty());
            // Request clipboard data in text format if available
            if context
                .remote_formats
                .contains(&RdpClipboardFormat::TextUnicode)
            {
                let format_data_request = freerdp_sys::CLIPRDR_FORMAT_DATA_REQUEST {
                    common: freerdp_sys::CLIPRDR_HEADER {
                        msgType: 0,
                        msgFlags: 0,
                        dataLen: std::mem::size_of::<freerdp_sys::CLIPRDR_FORMAT_DATA_REQUEST>()
                            as u32,
                    },
                    requestedFormatId: RdpClipboardFormat::TextUnicode.to_format_id(),
                };
                context.client_format_data_request(&format_data_request);
            }
        }
        freerdp_sys::CHANNEL_RC_OK
    }

    fn on_receive_format_data_request(
        &mut self,
        format_data_request: &freerdp_sys::CLIPRDR_FORMAT_DATA_REQUEST,
    ) -> u32 {
        if let Some(context) = self.channels.read().unwrap().cliprdr() {
            log::debug!(
                "Clipboard Receive Format Data Request event received in Rdp impl: {:?}",
                format_data_request
            );
            if let Some(format) =
                RdpClipboardFormat::from_format_id(format_data_request.requestedFormatId)
            {
                match format {
                    RdpClipboardFormat::TextUnicode => {
                        let clipboard_text = if let Some(ref clipboard_integration) =
                            self.config.integrations.clipboard
                        {
                            clipboard_integration.get_text().unwrap_or_default()
                        } else {
                            String::new()
                        };

                        log::debug!("Providing clipboard text data: {}", clipboard_text);

                        let mut text_bytes = clipboard_text.encode_utf16().collect::<Vec<u16>>();
                        text_bytes.push(0); // Add null terminator
                        let byte_len = (text_bytes.len() * 2) as u32;

                        let response_header = freerdp_sys::CLIPRDR_HEADER {
                            msgType: 0,
                            msgFlags: freerdp_sys::CB_RESPONSE_OK as u16,
                            dataLen: byte_len,
                        };

                        let format_data_response = freerdp_sys::CLIPRDR_FORMAT_DATA_RESPONSE {
                            common: response_header,
                            requestedFormatData: text_bytes.as_ptr() as *mut u8,
                        };

                        context.send_client_format_data_response(&format_data_response);
                    }
                    _ => {
                        log::warn!("Requested clipboard format not supported");
                    }
                }
            } else {
                log::warn!(
                    "Requested clipboard format id {} not recognized",
                    format_data_request.requestedFormatId
                );
            }
        }
        freerdp_sys::CHANNEL_RC_OK
    }

    // This is called by server in response to our format data request. It contains the actual clipboard data.
    fn on_receive_format_data_response(&mut self, data: &[u8]) -> u32 {
        log::debug!(
            "Clipboard Receive Format Data Response event received in Rdp impl: {:?}",
            data
        );
        // Assume data is UTF-16LE encoded text
        if !data.len().is_multiple_of(2) {
            log::warn!("Received clipboard data length is not even, cannot be valid UTF-16");
            return freerdp_sys::CHANNEL_RC_OK;
        }
        let u16_data: Vec<u16> = data
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        // Now to String, trimming null terminator if present
        let u16_data_trimmed = match u16_data.iter().position(|&v| v == 0) {
            Some(pos) => &u16_data[..pos],
            None => &u16_data,
        };

        if let Ok(text) = String::from_utf16(u16_data_trimmed) {
            log::debug!("Received clipboard text data: {}", text);
            // Set to local clipboard
            if let Some(ref clipboard_integration) = self.config.integrations.clipboard
                && let Err(e) = clipboard_integration.set_text(&text)
            {
                log::error!("Failed to set local clipboard text: {}", e);
            }
        } else {
            log::warn!("Failed to decode received clipboard data as UTF-16");
        }

        freerdp_sys::CHANNEL_RC_OK
    }
}
