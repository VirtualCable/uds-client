use std::sync::Arc;

use freerdp_sys::{
    CAM_MSG_ID_CAM_MSG_ID_SampleRequest, CAM_MSG_ID_CAM_MSG_ID_StartStreamsRequest, CHANNEL_RC_OK,
    IWTSVirtualChannel, IWTSVirtualChannelCallback, UINT,
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

    log::trace!(
        "Webcam PDU: version={version} msg_id={msg_id:?} len={}",
        bytes.len()
    );

    #[allow(non_upper_case_globals)]
    match msg_id {
        CAM_MSG_ID_CAM_MSG_ID_SampleRequest => {
            let stream_idx = payload.first().copied().unwrap_or(0);
            send_sample_response(ctx, stream_idx);
        }
        CAM_MSG_ID_CAM_MSG_ID_StartStreamsRequest => {
            handle_start_streams(ctx, payload);
        }
        _ => {}
    }

    CHANNEL_RC_OK
}

fn handle_start_streams(ctx: &ChannelCtx, payload: &[u8]) {
    // Payload: StreamIndex(1) + CAM_MEDIA_TYPE_DESCRIPTION(26)
    let stream_idx = payload.first().copied().unwrap_or(0);
    let Ok(mt) = parse_media_type(&payload[1..]) else {
        log::error!("Webcam: failed to parse media type");
        let err =
            pdu::build_response_header(freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_ErrorResponse as u32);
        unsafe { write_to_channel(ctx.channel, &err) };
        return;
    };

    let fps_num = mt.FrameRateNumerator;
    let fps_den = mt.FrameRateDenominator;
    let fps = fps_num.checked_div(fps_den).unwrap_or(15);

    log::info!(
        "Webcam: StartStreams — stream={stream_idx} format={:#x} {}x{} @ {fps_num}/{fps_den}={fps}fps",
        mt.Format,
        mt.Width,
        mt.Height,
    );

    ctx.webcam
        .set_format(mt.Format as u32, mt.Width, mt.Height, fps);

    let ok = pdu::build_response_header(freerdp_sys::CAM_MSG_ID_CAM_MSG_ID_SuccessResponse as u32);
    unsafe { write_to_channel(ctx.channel, &ok) };
}

fn send_sample_response(ctx: &ChannelCtx, stream_index: u8) {
    let Some(ref frame) = *ctx.webcam.latest_frame.lock().unwrap() else {
        return;
    };

    let pdu = pdu::build_sample_response(stream_index, frame);
    unsafe { write_to_channel(ctx.channel, &pdu) };
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
