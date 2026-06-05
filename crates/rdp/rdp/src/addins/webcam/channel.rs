use std::sync::Arc;

use freerdp_sys::{
    CHANNEL_RC_OK, IWTSVirtualChannel, IWTSVirtualChannelCallback, UINT,
};
use multimedia::webcam::WebcamHandle;
use shared::log;

use super::pdu::{self, parse_pdu_header, write_to_channel};

#[repr(C)]
pub struct ChannelCtx {
    pub channel_cb: IWTSVirtualChannelCallback,
    pub channel: *mut IWTSVirtualChannel,
    pub webcam: Arc<WebcamHandle>,
    pub stream_index: u8,
}

pub unsafe extern "C" fn on_data(
    cb: *mut IWTSVirtualChannelCallback,
    stream: *mut freerdp_sys::wStream,
) -> UINT {
    let ctx = unsafe { &*(cb as *const ChannelCtx) };
    if stream.is_null() {
        return CHANNEL_RC_OK;
    }

    let s = unsafe { &*stream };
    let bytes = unsafe { std::slice::from_raw_parts(s.pointer, s.length) };

    let Ok((version, msg_id, payload)) = parse_pdu_header(bytes) else {
        log::error!("Webcam: failed to parse PDU header!");
        return CHANNEL_RC_OK;
    };

    log::info!("Webcam PDU received: version={version} msg_id={msg_id} len={}", bytes.len());

    match msg_id as u32 {
        0x11 => { // CAM_MSG_ID_SampleRequest = 0x11
            ctx.webcam.request_sample();
        }
        0x0F => { // CAM_MSG_ID_StartStreamsRequest = 0x0F
            handle_start_streams(ctx, payload);
        }
        0x10 => { // CAM_MSG_ID_StopStreamsRequest = 0x10
            log::info!("Webcam: StopStreamsRequest");
            ctx.webcam.stop_stream();
            send_success(ctx);
        }
        0x07 => { // CAM_MSG_ID_ActivateDeviceRequest = 0x07
            log::info!("Webcam: ActivateDeviceRequest");
            send_success(ctx);
        }
        0x08 => { // CAM_MSG_ID_DeactivateDeviceRequest = 0x08
            log::info!("Webcam: DeactivateDeviceRequest");
            ctx.webcam.stop_stream();
            send_success(ctx);
        }
        0x09 => { // CAM_MSG_ID_StreamListRequest = 0x09
            handle_stream_list(ctx);
        }
        0x0B => { // CAM_MSG_ID_MediaTypeListRequest = 0x0B
            handle_media_type_list(ctx);
        }
        0x0D => { // CAM_MSG_ID_CurrentMediaTypeRequest = 0x0D
            handle_current_media_type(ctx);
        }
        0x14 => { // CAM_MSG_ID_PropertyListRequest = 0x14
            handle_property_list(ctx);
        }
        _ => {
            log::warn!("Webcam: unknown msg_id={msg_id}, sending error");
            send_error(ctx);
        }
    }

    CHANNEL_RC_OK
}

fn write_media_type(pdu: &mut Vec<u8>, format: u8, width: u32, height: u32, fps: u32) {
    pdu.push(format); // Format (1 byte)
    pdu.extend_from_slice(&width.to_le_bytes()); // Width (4 bytes)
    pdu.extend_from_slice(&height.to_le_bytes()); // Height (4 bytes)
    pdu.extend_from_slice(&fps.to_le_bytes()); // FrameRateNumerator (4 bytes)
    pdu.extend_from_slice(&1u32.to_le_bytes()); // FrameRateDenominator (4 bytes)
    pdu.extend_from_slice(&1u32.to_le_bytes()); // PixelAspectRatioNumerator (4 bytes)
    pdu.extend_from_slice(&1u32.to_le_bytes()); // PixelAspectRatioDenominator (4 bytes)
    pdu.push(1u8); // Flags (1 byte, CAM_MEDIA_TYPE_DESCRIPTION_FLAG_DecodingRequired = 1)
}

fn handle_start_streams(ctx: &ChannelCtx, payload: &[u8]) {
    if payload.len() < 27 {
        log::error!("Webcam: StartStreams payload too short: {}", payload.len());
        send_error(ctx);
        return;
    }
    let stream_idx = payload[0];
    let format = payload[1];
    
    let mut width_bytes = [0u8; 4];
    width_bytes.copy_from_slice(&payload[2..6]);
    let width = u32::from_le_bytes(width_bytes);

    let mut height_bytes = [0u8; 4];
    height_bytes.copy_from_slice(&payload[6..10]);
    let height = u32::from_le_bytes(height_bytes);

    let mut num_bytes = [0u8; 4];
    num_bytes.copy_from_slice(&payload[10..14]);
    let num = u32::from_le_bytes(num_bytes);

    let mut den_bytes = [0u8; 4];
    den_bytes.copy_from_slice(&payload[14..18]);
    let den = u32::from_le_bytes(den_bytes);

    let fps = num.checked_div(den).unwrap_or(15);

    log::info!(
        "Webcam: StartStreams — stream={stream_idx} format={format} {}x{} @ {fps}fps",
        width, height
    );

    // Set the webcam mode depending on format selected by server.
    // If server selected CAM_MEDIA_FORMAT_MJPG (2), encode as MJPEG.
    // Otherwise (e.g. YUY2, NV12, RGB24), send raw frame.
    if format == 2 {
        ctx.webcam.set_mode(multimedia::webcam::WebcamMode::MJPEG);
    } else {
        ctx.webcam.set_mode(multimedia::webcam::WebcamMode::Raw);
    }

    ctx.webcam.start_stream(width, height, fps);
    ctx.webcam.set_format(format as u32, width, height, fps);

    let channel_ptr = ctx.channel as usize;
    ctx.webcam.set_callback(move |frame| {
        log::info!("Webcam: Sending sample response of {} bytes, stream_idx={stream_idx}", frame.len());
        let pdu = pdu::build_sample_response(stream_idx, &frame);
        unsafe { pdu::write_to_channel(channel_ptr as *mut _, &pdu) };
    });

    send_success(ctx);
}

fn handle_stream_list(ctx: &ChannelCtx) {
    log::info!("Webcam: StreamListRequest — reporting 1 stream");
    // StreamListResponse: Header(2) + FrameSourceTypes(2) + StreamCategory(1) + Selected(1) + CanBeShared(1) = 7 bytes
    let mut pdu = Vec::with_capacity(7);
    pdu.push(1u8); // version
    pdu.push(0x0A); // msg_id (CAM_MSG_ID_StreamListResponse = 0x0A)
    
    // FrameSourceTypes: CAM_STREAM_FRAME_SOURCE_TYPE_Color = 0x0001 (2 bytes, LE)
    pdu.extend_from_slice(&1u16.to_le_bytes());
    // StreamCategory: CAM_STREAM_CATEGORY_Capture = 0x01 (1 byte)
    pdu.push(1u8);
    // Selected: TRUE = 1 (1 byte)
    pdu.push(1u8);
    // CanBeShared: FALSE = 0 (1 byte)
    pdu.push(0u8);

    unsafe { write_to_channel(ctx.channel, &pdu) };
}

fn handle_media_type_list(ctx: &ChannelCtx) {
    log::info!("Webcam: MediaTypeListRequest");
    // Respond with supported formats: MJPEG(2), YUYV(3), NV12(4), RGB24(6)
    let formats: [u8; 4] = [2, 3, 4, 6];
    let mut pdu = Vec::with_capacity(2 + formats.len() * 26);
    pdu.push(1u8); // version
    pdu.push(0x0C); // msg_id (CAM_MSG_ID_MediaTypeListResponse = 0x0C)

    for &fmt in &formats {
        write_media_type(&mut pdu, fmt, 640, 480, 30);
    }

    unsafe { write_to_channel(ctx.channel, &pdu) };
}

fn handle_current_media_type(ctx: &ChannelCtx) {
    log::info!("Webcam: CurrentMediaTypeRequest");
    let mut pdu = Vec::with_capacity(28);
    pdu.push(1u8); // version
    pdu.push(0x0E); // msg_id (CAM_MSG_ID_CurrentMediaTypeResponse = 0x0E)
    write_media_type(&mut pdu, 2, 640, 480, 30); // Default to MJPEG 640x480 @ 30fps

    unsafe { write_to_channel(ctx.channel, &pdu) };
}

fn handle_property_list(ctx: &ChannelCtx) {
    log::info!("Webcam: PropertyListRequest");
    // Header(2) + N_Properties(8) = 10 bytes
    let mut pdu = Vec::with_capacity(10);
    pdu.push(1u8); // version
    pdu.push(0x15); // msg_id (CAM_MSG_ID_PropertyListResponse = 0x15)
    pdu.extend_from_slice(&0u64.to_le_bytes()); // N_Properties = 0

    unsafe { write_to_channel(ctx.channel, &pdu) };
}


fn send_success(ctx: &ChannelCtx) {
    log::info!("Webcam: Sending SuccessResponse");
    let ok = pdu::build_response_header(
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_SuccessResponse as u8,
    );
    unsafe { write_to_channel(ctx.channel, &ok) };
}

fn send_error(ctx: &ChannelCtx) {
    log::error!("Webcam: Sending ErrorResponse");
    let err = pdu::build_response_header(
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_ErrorResponse as u8,
    );
    unsafe { write_to_channel(ctx.channel, &err) };
}

pub unsafe extern "C" fn on_close(cb: *mut IWTSVirtualChannelCallback) -> UINT {
    let ctx = cb as *mut ChannelCtx;
    log::info!("Webcam Device: Channel closed");

    unsafe {
        (*ctx).webcam.stop_stream();
        let _ = Box::from_raw(ctx);
    }
    CHANNEL_RC_OK
}
