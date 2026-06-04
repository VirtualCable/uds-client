use std::sync::Arc;

use freerdp_sys::{
    BOOL, BYTE, CHANNEL_RC_OK, IWTSListener, IWTSListenerCallback,
    IWTSVirtualChannel, IWTSVirtualChannelCallback, IWTSVirtualChannelManager, UINT,
};
use multimedia::webcam::WebcamHandle;
use shared::log;

use super::channel::{self, ChannelCtx};

#[repr(C)]
pub struct ListenerCtx {
    pub listener_cb: IWTSListenerCallback,
    pub webcam: Arc<WebcamHandle>,
}

pub unsafe extern "C" fn on_new_channel(
    listener_cb: *mut IWTSListenerCallback,
    p_channel: *mut IWTSVirtualChannel,
    _data: *mut BYTE,
    pb_accept: *mut BOOL,
    pp_callback: *mut *mut IWTSVirtualChannelCallback,
) -> UINT {
    let lctx = listener_cb as *mut ListenerCtx;
    let webcam = unsafe { (*lctx).webcam.clone() };

    log::info!("Webcam: OnNewChannelConnection — channel={p_channel:?}, accepting");

    unsafe {
        *pb_accept = true.into();
    }

    let mut channel_ctx = Box::new(ChannelCtx {
        channel_cb: IWTSVirtualChannelCallback {
            OnDataReceived: Some(channel::on_data),
            OnClose: Some(channel::on_close),
            ..unsafe { std::mem::zeroed() }
        },
        channel: p_channel,
        webcam,
        stream_index: 0,
    });

    channel_ctx.webcam.start_stream(640, 480, 15);

    unsafe {
        *pp_callback = &mut channel_ctx.channel_cb;
    }

    let _ = Box::into_raw(channel_ctx);
    CHANNEL_RC_OK
}

pub(super) fn create_listener(
    webcam: Arc<WebcamHandle>,
    channel_mgr: *mut IWTSVirtualChannelManager,
) -> (*mut ListenerCtx, *mut IWTSListener, UINT) {
    let mut listener_ctx = Box::new(ListenerCtx {
        listener_cb: IWTSListenerCallback {
            OnNewChannelConnection: Some(on_new_channel),
            pInterface: Arc::as_ptr(&webcam) as *mut _,
        },
        webcam,
    });

    let mut listener_handle: *mut IWTSListener = std::ptr::null_mut();
    let error = unsafe {
        (*channel_mgr).CreateListener.unwrap_unchecked()(
            channel_mgr,
            c"RDCamera_Device_Enumerator".as_ptr(),
            0,
            &mut listener_ctx.listener_cb,
            &mut listener_handle,
        )
    };

    let raw_ctx = Box::into_raw(listener_ctx);
    (raw_ctx, listener_handle, error)
}
