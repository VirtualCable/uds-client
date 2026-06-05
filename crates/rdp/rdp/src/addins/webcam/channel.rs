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

    let Ok((_version, msg_id, payload)) = parse_pdu_header(bytes) else {
        log::error!("Webcam: failed to parse PDU header!");
        return CHANNEL_RC_OK;
    };

    #[allow(non_upper_case_globals)]
    match msg_id as i32 {
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_SampleRequest => {
            ctx.webcam.request_sample(ctx.channel as usize);
        }
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_StartStreamsRequest => {
            handle_start_streams(ctx, payload);
        }
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_StopStreamsRequest => {
            log::info!("Webcam: StopStreamsRequest");
            ctx.webcam.stop_stream();
            send_success(ctx);
        }
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_ActivateDeviceRequest => {
            log::info!("Webcam: ActivateDeviceRequest");
            send_success(ctx);
        }
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_DeactivateDeviceRequest => {
            log::info!("Webcam: DeactivateDeviceRequest");
            ctx.webcam.stop_stream();
            send_success(ctx);
        }
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_StreamListRequest => {
            handle_stream_list(ctx);
        }
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_MediaTypeListRequest => {
            handle_media_type_list(ctx);
        }
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_CurrentMediaTypeRequest => {
            handle_current_media_type(ctx);
        }
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_PropertyListRequest => {
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
    let mt = freerdp_sys::CAM_MEDIA_TYPE_DESCRIPTION {
        Format: format as i32,
        Width: width,
        Height: height,
        FrameRateNumerator: fps,
        FrameRateDenominator: 1,
        PixelAspectRatioNumerator: 1,
        PixelAspectRatioDenominator: 1,
        Flags: freerdp_sys::CAM_MEDIA_TYPE_DESCRIPTION_FLAGS_CAM_MEDIA_TYPE_DESCRIPTION_FLAG_DecodingRequired,
    };
    pdu.extend_from_slice(&pdu::serialize_media_type(&mt));
}

fn handle_start_streams(ctx: &ChannelCtx, payload: &[u8]) {
    if payload.len() < 27 {
        log::error!("Webcam: StartStreams payload too short: {}", payload.len());
        send_error(ctx);
        return;
    }
    let stream_idx = payload[0];
    let media_type = match pdu::parse_media_type(&payload[1..]) {
        Ok(mt) => mt,
        Err(e) => {
            log::error!("Webcam: failed to parse media type: {e}");
            send_error(ctx);
            return;
        }
    };
    let format = media_type.Format;
    let width = media_type.Width;
    let height = media_type.Height;
    let fps = media_type.FrameRateNumerator.checked_div(media_type.FrameRateDenominator).unwrap_or(15);

    log::info!(
        "Webcam: StartStreams — stream={stream_idx} format={format} {}x{} @ {fps}fps",
        width, height
    );

    // Set the webcam mode depending on format selected by server.
    // If server selected CAM_MEDIA_FORMAT_MJPG (2), encode as MJPEG.
    // Otherwise (e.g. YUY2, NV12, RGB24), send raw frame.
    if format == freerdp_sys::CAM_MEDIA_FORMAT_CAM_MEDIA_FORMAT_MJPG {
        ctx.webcam.set_mode(multimedia::webcam::WebcamMode::MJPEG);
    } else if format == freerdp_sys::CAM_MEDIA_FORMAT_CAM_MEDIA_FORMAT_YUY2 {
        ctx.webcam.set_mode(multimedia::webcam::WebcamMode::YUY2);
    } else {
        ctx.webcam.set_mode(multimedia::webcam::WebcamMode::Raw);
    }

    ctx.webcam.start_stream(width, height, fps);
    ctx.webcam.set_format(format as u32, width, height, fps);

    let (tx, rx) = flume::unbounded::<multimedia::webcam::WebcamFrame>();
    ctx.webcam.set_sender(tx);

    std::thread::spawn(move || {
        log::info!("Webcam: Frame writer thread started for stream {stream_idx}");
        while let Ok(frame) = rx.recv() {
            log::info!("Webcam: Sending sample response of {} bytes, stream_idx={stream_idx}", frame.data.len());
            let pdu = pdu::build_sample_response(stream_idx, &frame.data);
            unsafe { pdu::write_to_channel(frame.channel_ptr as *mut _, &pdu) };
        }
        log::info!("Webcam: Frame writer thread stopped for stream {stream_idx}");
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
