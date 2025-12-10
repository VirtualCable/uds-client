use shared::log;

use crate::{Rdp, channels::cliprdr::traits::ClipboardHandler, context::RdpContext};

fn get_owner<'a>(context: *mut freerdp_sys::CliprdrClientContext) -> Option<&'a mut Rdp> {
    if context.is_null() {
        log::error!("CliprdrClientContext is null");
        return None;
    }

    let owner_ptr = unsafe { (*context).custom };
    if owner_ptr.is_null() {
        log::error!("CliprdrClientContext.custom (owner) is null");
        return None;
    }

    let rdp_context = unsafe { &mut *(owner_ptr as *mut RdpContext) };
    unsafe { rdp_context.owner.as_mut() }
}

unsafe extern "C" fn monitor_ready(
    context: *mut freerdp_sys::CliprdrClientContext,
    monitor_ready: *const freerdp_sys::CLIPRDR_MONITOR_READY,
) -> freerdp_sys::UINT {
    log::debug!(
        "Clipboard Monitor Ready callback called: context={:?}, monitor_ready={:?}",
        context,
        monitor_ready
    );
    if let Some(rdp) = get_owner(context) {
        return rdp.on_monitor_ready(unsafe { &*monitor_ready });
    }
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
    if let Some(rdp) = get_owner(context) {
        return rdp.on_receive_server_capabilities(unsafe { &*capabilities });
    }
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
    if let Some(rdp) = get_owner(context) {
        return rdp.on_receive_server_format_list(unsafe { &*format_list });
    }
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
    if let Some(rdp) = get_owner(context) {
        // If not failed, send success. No other data is needed.
        return rdp.on_receive_format_list_response(unsafe {
            (*format_list_response).common.msgFlags & freerdp_sys::CB_RESPONSE_FAIL as u16 == 0
        });
    }
    freerdp_sys::CHANNEL_RC_OK
}

// Invoked by serer to request clipboard data in specific format
unsafe extern "C" fn receive_format_data_request(
    context: *mut freerdp_sys::CliprdrClientContext,
    format_data_request: *const freerdp_sys::CLIPRDR_FORMAT_DATA_REQUEST,
) -> freerdp_sys::UINT {
    log::debug!(
        "Clipboard Receive Format Data Request callback called: context={:?}, format_data_request={:?}",
        context,
        format_data_request
    );
    if let Some(rdp) = get_owner(context) {
        return rdp.on_receive_format_data_request(unsafe { &*format_data_request });
    }
    freerdp_sys::CHANNEL_RC_OK
}

// Invoked by server to send clipboard data in response to a client format data request (sent by us)
unsafe extern "C" fn receive_format_data_response(
    context: *mut freerdp_sys::CliprdrClientContext,
    format_data_response: *const freerdp_sys::CLIPRDR_FORMAT_DATA_RESPONSE,
) -> freerdp_sys::UINT {
    log::debug!(
        "Clipboard Receive Format Data Response callback called: context={:?}, format_data_response={:?}",
        context,
        format_data_response
    );
    // Note: Currently we only request text data, so we expect text here
    if let Some(rdp) = get_owner(context) {
        // Compose a const slice from buffer pointer and length
        let data = unsafe {
            if !(*format_data_response).requestedFormatData.is_null()
                && (*format_data_response).common.dataLen > 0
            {
                std::slice::from_raw_parts(
                    (*format_data_response).requestedFormatData,
                    (*format_data_response).common.dataLen as usize,
                )
            } else {
                &[]
            }
        };
        return rdp.on_receive_format_data_response(data);
    }
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
