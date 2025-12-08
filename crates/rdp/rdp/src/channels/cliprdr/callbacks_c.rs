use shared::log;

unsafe extern "C" fn monitor_ready(
    context: *mut freerdp_sys::CliprdrClientContext,
    monitor_ready: *const freerdp_sys::CLIPRDR_MONITOR_READY,
) -> freerdp_sys::UINT {
    log::debug!(
        "Clipboard Monitor Ready callback called: context={:?}, monitor_ready={:?}",
        context,
        monitor_ready
    );
    freerdp_sys::CHANNEL_RC_OK
}

unsafe extern "C" fn receive_server_capabilities(
    context: *mut freerdp_sys::CliprdrClientContext,
    capabilities: *const freerdp_sys::CLIPRDR_CAPABILITIES,
) -> freerdp_sys::UINT {
    log::debug!(
        "Clipboard Receive Server Capabilities callback called: context={:?}, capabilities={:?}",
        context,
        capabilities
    );
    freerdp_sys::CHANNEL_RC_OK
}

unsafe extern "C" fn receive_server_format_list(
    context: *mut freerdp_sys::CliprdrClientContext,
    format_list: *const freerdp_sys::CLIPRDR_FORMAT_LIST,
) -> freerdp_sys::UINT {
    log::debug!(
        "Clipboard Receive Server Format List callback called: context={:?}, format_list={:?}",
        context,
        format_list
    );
    freerdp_sys::CHANNEL_RC_OK
}

unsafe extern "C" fn receive_format_list_response(
    context: *mut freerdp_sys::CliprdrClientContext,
    format_list_response: *const freerdp_sys::CLIPRDR_FORMAT_LIST_RESPONSE,
) -> freerdp_sys::UINT {
    log::debug!(
        "Clipboard Receive Format List Response callback called: context={:?}, format_list_response={:?}",
        context,
        format_list_response
    );
    freerdp_sys::CHANNEL_RC_OK
}

unsafe extern "C" fn receive_format_data_request(
    context: *mut freerdp_sys::CliprdrClientContext,
    format_data_request: *const freerdp_sys::CLIPRDR_FORMAT_DATA_REQUEST,
) -> freerdp_sys::UINT {
    log::debug!(
        "Clipboard Receive Format Data Request callback called: context={:?}, format_data_request={:?}",
        context,
        format_data_request
    );
    freerdp_sys::CHANNEL_RC_OK
}

unsafe extern "C" fn receive_format_data_response(
    context: *mut freerdp_sys::CliprdrClientContext,
    format_data_response: *const freerdp_sys::CLIPRDR_FORMAT_DATA_RESPONSE,
) -> freerdp_sys::UINT {
    log::debug!(
        "Clipboard Receive Format Data Response callback called: context={:?}, format_data_response={:?}",
        context,
        format_data_response
    );
    freerdp_sys::CHANNEL_RC_OK
}

pub fn register_cliprdr_callbacks(cliprdr: &mut freerdp_sys::CliprdrClientContext) {
    cliprdr.MonitorReady = Some(monitor_ready);
    cliprdr.ServerCapabilities = Some(receive_server_capabilities);
    cliprdr.ServerFormatList = Some(receive_server_format_list);
    cliprdr.ServerFormatListResponse = Some(receive_format_list_response);
    cliprdr.ServerFormatDataRequest = Some(receive_format_data_request);
    cliprdr.ServerFormatDataResponse = Some(receive_format_data_response);
}
