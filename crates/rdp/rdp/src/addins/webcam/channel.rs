use std::sync::Arc;

use freerdp_sys::{
    CAM_MSG_ID_CAM_MSG_ID_ActivateDeviceRequest,
    CAM_MSG_ID_CAM_MSG_ID_CurrentMediaTypeRequest,
    CAM_MSG_ID_CAM_MSG_ID_DeactivateDeviceRequest,
    CAM_MSG_ID_CAM_MSG_ID_MediaTypeListRequest,
    CAM_MSG_ID_CAM_MSG_ID_PropertyListRequest,
    CAM_MSG_ID_CAM_MSG_ID_SampleRequest,
    CAM_MSG_ID_CAM_MSG_ID_StartStreamsRequest,
    CAM_MSG_ID_CAM_MSG_ID_StopStreamsRequest,
    CAM_MSG_ID_CAM_MSG_ID_StreamListRequest,
    CHANNEL_RC_OK, IWTSVirtualChannel, IWTSVirtualChannelCallback, UINT,
};
use multimedia::webcam::WebcamHandle;
use shared::log;

use super::pdu::{self, parse_media_type, parse_pdu_header, write_to_channel};

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
        return CHANNEL_RC_OK;
    };

    log::trace!("Webcam PDU: version={version} msg_id={msg_id:?} len={}", bytes.len());

    #[allow(non_upper_case_globals)]
    match msg_id {
        CAM_MSG_ID_CAM_MSG_ID_SampleRequest => {
            let stream_idx = payload.first().copied().unwrap_or(0);
            send_sample_response(ctx, stream_idx);
        }
        CAM_MSG_ID_CAM_MSG_ID_StartStreamsRequest => {
            handle_start_streams(ctx, payload);
        }
        CAM_MSG_ID_CAM_MSG_ID_StopStreamsRequest => {
            log::info!("Webcam: StopStreamsRequest");
            ctx.webcam.stop_stream();
            send_success(ctx);
        }
        CAM_MSG_ID_CAM_MSG_ID_ActivateDeviceRequest => {
            log::info!("Webcam: ActivateDeviceRequest");
            send_success(ctx);
        }
        CAM_MSG_ID_CAM_MSG_ID_DeactivateDeviceRequest => {
            log::info!("Webcam: DeactivateDeviceRequest");
            ctx.webcam.stop_stream();
            send_success(ctx);
        }
        CAM_MSG_ID_CAM_MSG_ID_StreamListRequest => {
            handle_stream_list(ctx);
        }
        CAM_MSG_ID_CAM_MSG_ID_MediaTypeListRequest => {
            handle_media_type_list(ctx);
        }
        CAM_MSG_ID_CAM_MSG_ID_CurrentMediaTypeRequest => {
            handle_current_media_type(ctx);
        }
        CAM_MSG_ID_CAM_MSG_ID_PropertyListRequest => {
            handle_property_list(ctx);
        }
        _ => {
            log::warn!("Webcam: unknown msg_id={msg_id:?}, sending error");
            send_error(ctx);
        }
    }

    CHANNEL_RC_OK
}

fn handle_start_streams(ctx: &ChannelCtx, payload: &[u8]) {
    let stream_idx = payload.first().copied().unwrap_or(0);
    let Ok(mt) = parse_media_type(&payload[1..]) else {
        log::error!("Webcam: failed to parse media type");
        send_error(ctx);
        return;
    };

    let fps = mt.FrameRateNumerator
        .checked_div(mt.FrameRateDenominator)
        .unwrap_or(15);

    log::info!(
        "Webcam: StartStreams — stream={stream_idx} format={:#x} {}x{} @ {fps}fps",
        mt.Format, mt.Width, mt.Height,
    );

    ctx.webcam.set_format(mt.Format as u32, mt.Width, mt.Height, fps);
    send_success(ctx);
}

fn handle_stream_list(ctx: &ChannelCtx) {
    log::info!("Webcam: StreamListRequest — reporting 1 stream");
    // StreamListResponse: Header(5) + N_Descriptions(1) + StreamDescription(12)
    let mut pdu = Vec::with_capacity(pdu::PDU_HEADER_SIZE + 1 + 12);
    pdu.push(1u8);
    pdu.extend_from_slice(
        &(freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_StreamListResponse as u32).to_le_bytes(),
    );
    pdu.push(1u8); // N_Descriptions = 1 stream
    // CAM_STREAM_DESCRIPTION: StreamIndex(1) + Category(1) + StreamNameLen(4) + Name(6) = 12 bytes
    pdu.push(0u8);         // StreamIndex
    pdu.push(0u8);         // Category: unspecified
    pdu.extend_from_slice(&6u32.to_le_bytes()); // StreamNameLen = 6 (L"Video\0")
    b"Video\0".iter().for_each(|&b| pdu.push(b)); // StreamName (wide-char, just ASCII for simplicity)
    unsafe { write_to_channel(ctx.channel, &pdu) };
}

fn handle_media_type_list(ctx: &ChannelCtx) {
    log::info!("Webcam: MediaTypeListRequest");
    // Respond with our supported formats: MJPEG(2), YUYV(3), NV12(4), RGB24(6)
    // Each CAM_MEDIA_TYPE_DESCRIPTION = 26 bytes
    let formats: [u32; 4] = [2, 3, 4, 6]; // MJPG, YUY2, NV12, RGB24
    let n = formats.len();

    let body_size = n * 26;
    let pdu_size = pdu::PDU_HEADER_SIZE + 4 + body_size; // +4 for N_Descriptions(usize)
    let mut pdu = Vec::with_capacity(pdu_size);

    pdu.push(1u8);
    pdu.extend_from_slice(
        &(freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_MediaTypeListResponse as u32).to_le_bytes(),
    );
    pdu.extend_from_slice(&(n as u64).to_le_bytes()); // N_Descriptions (usize → u64 LE)

    for &fmt in &formats {
        // Build a CAM_MEDIA_TYPE_DESCRIPTION for each format
        let mt = freerdp_sys::CAM_MEDIA_TYPE_DESCRIPTION {
            Format: fmt as i32,
            Width: 640,
            Height: 480,
            FrameRateNumerator: 30,
            FrameRateDenominator: 1,
            PixelAspectRatioNumerator: 1,
            PixelAspectRatioDenominator: 1,
            Flags: 0,
        };
        let mt_bytes = unsafe {
            std::slice::from_raw_parts(
                &mt as *const _ as *const u8,
                std::mem::size_of::<freerdp_sys::CAM_MEDIA_TYPE_DESCRIPTION>(),
            )
        };
        pdu.extend_from_slice(mt_bytes);
    }

    unsafe { write_to_channel(ctx.channel, &pdu) };
}

fn handle_current_media_type(ctx: &ChannelCtx) {
    log::info!("Webcam: CurrentMediaTypeRequest");
    // Respond with a default MJPEG 640x480 @ 30fps media type
    let mt = freerdp_sys::CAM_MEDIA_TYPE_DESCRIPTION {
        Format: 2,  // MJPEG
        Width: 640,
        Height: 480,
        FrameRateNumerator: 30,
        FrameRateDenominator: 1,
        PixelAspectRatioNumerator: 1,
        PixelAspectRatioDenominator: 1,
        Flags: 0,
    };
    let mt_bytes = unsafe {
        std::slice::from_raw_parts(
            &mt as *const _ as *const u8,
            std::mem::size_of::<freerdp_sys::CAM_MEDIA_TYPE_DESCRIPTION>(),
        )
    };

    let mut pdu = Vec::with_capacity(pdu::PDU_HEADER_SIZE + mt_bytes.len());
    pdu.push(1u8);
    pdu.extend_from_slice(
        &(freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_CurrentMediaTypeResponse as u32)
            .to_le_bytes(),
    );
    pdu.extend_from_slice(mt_bytes);
    unsafe { write_to_channel(ctx.channel, &pdu) };
}

fn handle_property_list(ctx: &ChannelCtx) {
    log::info!("Webcam: PropertyListRequest");
    // Respond with 0 properties (not supported)
    let mut pdu = Vec::with_capacity(pdu::PDU_HEADER_SIZE + 8);
    pdu.push(1u8);
    pdu.extend_from_slice(
        &(freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_PropertyListResponse as u32).to_le_bytes(),
    );
    pdu.extend_from_slice(&0u64.to_le_bytes()); // N_Properties = 0
    unsafe { write_to_channel(ctx.channel, &pdu) };
}

fn send_sample_response(ctx: &ChannelCtx, stream_index: u8) {
    let Some(ref frame) = *ctx.webcam.latest_frame.lock().unwrap() else {
        return;
    };
    let pdu = pdu::build_sample_response(stream_index, frame);
    unsafe { write_to_channel(ctx.channel, &pdu) };
}

fn send_success(ctx: &ChannelCtx) {
    let ok = pdu::build_response_header(
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_SuccessResponse as u32,
    );
    unsafe { write_to_channel(ctx.channel, &ok) };
}

fn send_error(ctx: &ChannelCtx) {
    let err = pdu::build_response_header(
        freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_ErrorResponse as u32,
    );
    unsafe { write_to_channel(ctx.channel, &err) };
}

pub unsafe extern "C" fn on_close(cb: *mut IWTSVirtualChannelCallback) -> UINT {
    let ctx = cb as *mut ChannelCtx;
    log::info!("Webcam: Channel closed");

    unsafe {
        (*ctx).webcam.stop_stream();
        let _ = Box::from_raw(ctx);
    }
    CHANNEL_RC_OK
}
