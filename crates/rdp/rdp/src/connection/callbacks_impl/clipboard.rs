
use shared::log;

use super::Rdp;

use crate::channels::cliprdr::ClipboardHandler;


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
            
            context.client_capabilities(&capabilities);

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

            context.client_format_list(&format_list);

            freerdp_sys::CHANNEL_RC_OK
        } else {
            log::error!("Clipboard context is null in Monitor Ready");
            freerdp_sys::CHANNEL_RC_TOO_MANY_CHANNELS
        }
    }
}
