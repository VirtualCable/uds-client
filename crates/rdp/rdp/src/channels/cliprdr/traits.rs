
use shared::log;

pub use freerdp_sys::CHANNEL_RC_OK;

pub trait ClipboardHandler {
    fn on_monitor_ready(&mut self, monitor_ready: &freerdp_sys::CLIPRDR_MONITOR_READY) -> u32 {
        log::debug!(
            "Clipboard Monitor Ready event received: {:?}",
            monitor_ready
        );
        CHANNEL_RC_OK
    }
    fn on_receive_server_capabilities(
        &mut self,
        capabilities: &freerdp_sys::CLIPRDR_CAPABILITIES,
    ) -> u32 {
        log::debug!(
            "Clipboard Receive Server Capabilities event received: {:?}",
            capabilities
        );
        CHANNEL_RC_OK
    }
    fn on_receive_server_format_list(
        &mut self,
        format_list: &freerdp_sys::CLIPRDR_FORMAT_LIST,
    ) -> u32 {
        log::debug!(
            "Clipboard Receive Server Format List event received: {:?}",
            format_list
        );
        CHANNEL_RC_OK
    }
    fn on_receive_format_list_response(
        &mut self,
        format_list_response: &freerdp_sys::CLIPRDR_FORMAT_LIST_RESPONSE,
    ) -> u32 {
        log::debug!(
            "Clipboard Receive Format List Response event received: {:?}",
            format_list_response
        );
        CHANNEL_RC_OK
    }
    fn on_receive_format_data_request(
        &mut self,
        format_data_request: &freerdp_sys::CLIPRDR_FORMAT_DATA_REQUEST,
    ) -> u32 {
        log::debug!(
            "Clipboard Receive Format Data Request event received: {:?}",
            format_data_request
        );
        CHANNEL_RC_OK
    }
    fn on_receive_format_data_response(
        &mut self,
        format_data_response: &freerdp_sys::CLIPRDR_FORMAT_DATA_RESPONSE,
    ) -> u32 {
        log::debug!(
            "Clipboard Receive Format Data Response event received: {:?}",
            format_data_response
        );
        CHANNEL_RC_OK
    }
}
