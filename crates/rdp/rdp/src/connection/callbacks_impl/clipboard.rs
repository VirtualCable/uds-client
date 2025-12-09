use std::collections::HashSet;

use shared::log;

use super::Rdp;

use crate::channels::cliprdr::{ClipboardFormat, ClipboardHandler};

impl ClipboardHandler for Rdp {
    // TODO: implement clipboard handling methods here
    fn on_monitor_ready(&mut self, monitor_ready: &freerdp_sys::CLIPRDR_MONITOR_READY) -> u32 {
        log::debug!(
            "Clipboard Monitor Ready event received in Rdp impl: {:?}",
            monitor_ready
        );
        if let Some(context) = self.channels.read().unwrap().cliprdr() {
            let general_capability_set = freerdp_sys::CLIPRDR_GENERAL_CAPABILITY_SET {
                capabilitySetType: freerdp_sys::CB_CAPSTYPE_GENERAL as u16,
                capabilitySetLength: std::mem::size_of::<freerdp_sys::CLIPRDR_GENERAL_CAPABILITY_SET>(
                ) as u16,
                version: freerdp_sys::CB_CAPS_VERSION_2,
                generalFlags: freerdp_sys::CB_USE_LONG_FORMAT_NAMES, // Just this for text
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

            let result = context.client_capabilities(&capabilities);
            log::debug!("Sent clipboard capabilities, result: {}", result);

            let text_formats = [
                freerdp_sys::CLIPRDR_FORMAT {
                    formatId: freerdp_sys::CF_UNICODETEXT,
                    formatName: std::ptr::null_mut(),
                },
                freerdp_sys::CLIPRDR_FORMAT {
                    formatId: freerdp_sys::CF_TEXT,
                    formatName: std::ptr::null_mut(),
                },
            ];

            let format_list = freerdp_sys::CLIPRDR_FORMAT_LIST {
                common: freerdp_sys::CLIPRDR_HEADER {
                    msgType: 0,
                    msgFlags: 0,
                    dataLen: 0,
                },
                numFormats: text_formats.len() as u32,
                formats: text_formats.as_ptr() as *mut _,
            };

            let result = context.client_format_list(&format_list);
            log::debug!("Sent clipboard format list, result: {}", result);

            freerdp_sys::CHANNEL_RC_OK
        } else {
            log::error!("Clipboard context is null in Monitor Ready");
            freerdp_sys::CHANNEL_RC_TOO_MANY_CHANNELS
        }
    }

    fn on_receive_format_list_response(
        &mut self,
        format_list_response: &freerdp_sys::CLIPRDR_FORMAT_LIST_RESPONSE,
    ) -> u32 {
        log::debug!(
            "Clipboard Receive Format List Response event received in Rdp impl: {:?}",
            format_list_response
        );
        if format_list_response.common.msgFlags & freerdp_sys::CB_RESPONSE_FAIL as u16 != 0 {
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

    fn on_receive_server_format_list(
        &mut self,
        format_list: &freerdp_sys::CLIPRDR_FORMAT_LIST,
    ) -> u32 {
        if let Some(mut context) = self.channels.read().unwrap().cliprdr() {
            log::debug!(
                "Clipboard Receive Server Format List event received in Rdp impl: {:?}",
                format_list
            );
            context.formats.clear();
            unsafe {
                let formats_ptr = format_list.formats as *const freerdp_sys::CLIPRDR_FORMAT;
                let mut supported: HashSet<ClipboardFormat> = HashSet::new();
                for i in 0..format_list.numFormats {
                    let format = &*formats_ptr.add(i as usize);
                    if let Some(clip_format) = ClipboardFormat::from_format(format) {
                        supported.insert(clip_format);
                    }
                }

                // Note: We are only interested right now on text formats
                context.formats = supported.into_iter().collect(); // Convert HashSet to Vec

                log::debug!("Supported clipboard formats: {:?}", context.formats);
            }
            context.send_format_list_response(!context.formats.is_empty());
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
            if let Some(format) = ClipboardFormat::from_format_id(format_data_request.requestedFormatId) {
                match format {
                    ClipboardFormat::Text => {
                        // Here we would retrieve the clipboard text from the local system
                        let clipboard_text = "Testing clipboard text from RDP client";

                        let text_bytes = clipboard_text.encode_utf16().collect::<Vec<u16>>();
                        let byte_len = ((text_bytes.len() + 1) * 2) as u32;  // +1 for null terminator

                        let response_header = freerdp_sys::CLIPRDR_HEADER {
                            msgType: 0,
                            msgFlags: freerdp_sys::CB_RESPONSE_OK as u16,
                            dataLen: byte_len,
                        };

                        let format_data_response = freerdp_sys::CLIPRDR_FORMAT_DATA_RESPONSE {
                            common: response_header,
                            requestedFormatData: text_bytes.as_ptr() as *mut u8,
                        };

                        context.send_format_data_response(&format_data_response);
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
        // Check if the requested format is text, currently only text is supported
        freerdp_sys::CHANNEL_RC_OK
    }

    fn on_receive_format_data_response(
        &mut self,
        format_data_response: &freerdp_sys::CLIPRDR_FORMAT_DATA_RESPONSE,
    ) -> u32 {
        log::debug!(
            "Clipboard Receive Format Data Response event received in Rdp impl: {:?}",
            format_data_response
        );
        freerdp_sys::CHANNEL_RC_OK
    }
}
