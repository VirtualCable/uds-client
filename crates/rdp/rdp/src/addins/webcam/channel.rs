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
    // FrameSourceTypes: CAM_STREAM_FRAME_SOURCE_TYPE_Color = 0x0001 (1u16)
    // StreamCategory: CAM_STREAM_CATEGORY_Capture = 0x01 (1u8)
    let pdu = pdu::build_stream_list_response(1, 1, true, false);
    unsafe { write_to_channel(ctx.channel, &pdu) };
}

fn handle_media_type_list(ctx: &ChannelCtx) {
    log::info!("Webcam: MediaTypeListRequest");
    let mt = freerdp_sys::CAM_MEDIA_TYPE_DESCRIPTION {
        Format: freerdp_sys::CAM_MEDIA_FORMAT_CAM_MEDIA_FORMAT_MJPG,
        Width: 640,
        Height: 480,
        FrameRateNumerator: 30,
        FrameRateDenominator: 1,
        PixelAspectRatioNumerator: 1,
        PixelAspectRatioDenominator: 1,
        Flags: freerdp_sys::CAM_MEDIA_TYPE_DESCRIPTION_FLAGS_CAM_MEDIA_TYPE_DESCRIPTION_FLAG_DecodingRequired,
    };
    let pdu = pdu::build_media_type_list_response(&[mt]);
    unsafe { write_to_channel(ctx.channel, &pdu) };
}

fn handle_current_media_type(ctx: &ChannelCtx) {
    log::info!("Webcam: CurrentMediaTypeRequest");
    let mt = freerdp_sys::CAM_MEDIA_TYPE_DESCRIPTION {
        Format: freerdp_sys::CAM_MEDIA_FORMAT_CAM_MEDIA_FORMAT_MJPG,
        Width: 640,
        Height: 480,
        FrameRateNumerator: 30,
        FrameRateDenominator: 1,
        PixelAspectRatioNumerator: 1,
        PixelAspectRatioDenominator: 1,
        Flags: freerdp_sys::CAM_MEDIA_TYPE_DESCRIPTION_FLAGS_CAM_MEDIA_TYPE_DESCRIPTION_FLAG_DecodingRequired,
    };
    let pdu = pdu::build_current_media_type_response(&mt);
    unsafe { write_to_channel(ctx.channel, &pdu) };
}

fn handle_property_list(ctx: &ChannelCtx) {
    log::info!("Webcam: PropertyListRequest");
    let pdu = pdu::build_property_list_response();
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
