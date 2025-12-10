use std::collections::HashSet;

use shared::log;

use super::Rdp;

use crate::channels::cliprdr::{ClipboardFormat, ClipboardHandler};

impl ClipboardHandler for Rdp {
    fn on_monitor_ready(&mut self, monitor_ready: &freerdp_sys::CLIPRDR_MONITOR_READY) -> u32 {
        log::debug!(
            "Clipboard Monitor Ready event received in Rdp impl: {:?}",
            monitor_ready
        );
        if let Some(context) = self.channels.read().unwrap().cliprdr() {
            // Server expect capabilities + initial clipboard status available
            context.send_client_capabilities(freerdp_sys::CB_USE_LONG_FORMAT_NAMES);
            // TODO: We have no initial data, but set to unicode here to make some tests work
            // Remove it and leave empty until we have real clipboard data
            context.send_client_format_list(&[ClipboardFormat::TextUnicode]);

            freerdp_sys::CHANNEL_RC_OK
        } else {
            log::error!("Clipboard context is null in Monitor Ready");
            freerdp_sys::CHANNEL_RC_TOO_MANY_CHANNELS
        }
    }

    fn on_receive_format_list_response(
        &mut self,
        success: bool,
    ) -> u32 {
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

                log::debug!("Supported clipboard formats from remote: {:?}", context.formats);
            }
            context.send_format_list_response(!context.formats.is_empty());
            // Request clipboard data in text format if available
            if context.formats.contains(&ClipboardFormat::TextUnicode) {
                let format_data_request = freerdp_sys::CLIPRDR_FORMAT_DATA_REQUEST {
                    common: freerdp_sys::CLIPRDR_HEADER {
                        msgType: 0,
                        msgFlags: 0,
                        dataLen: std::mem::size_of::<freerdp_sys::CLIPRDR_FORMAT_DATA_REQUEST>()
                            as u32,
                    },
                    requestedFormatId: ClipboardFormat::TextUnicode.to_format_id(),
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
                ClipboardFormat::from_format_id(format_data_request.requestedFormatId)
            {
                match format {
                    ClipboardFormat::TextUnicode => {
                        // Here we would retrieve the clipboard text from the local system
                        let clipboard_text = "Testing clipboard text from RDP client";

                        let text_bytes = clipboard_text.encode_utf16().collect::<Vec<u16>>();
                        let byte_len = ((text_bytes.len() + 1) * 2) as u32; // +1 for null terminator

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
        // Check if the requested format is text, currently only text is supported
        freerdp_sys::CHANNEL_RC_OK
    }

    // This is called by server in response to our format data request. It contains the actual clipboard data.
    fn on_receive_format_data_response(
        &mut self,
        data: &[u8],
    ) -> u32 {
        log::debug!(
            "Clipboard Receive Format Data Response event received in Rdp impl: {:?}",
            data
        );
        freerdp_sys::CHANNEL_RC_OK
    }
}
