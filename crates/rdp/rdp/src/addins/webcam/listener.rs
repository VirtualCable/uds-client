// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::integrations::WebcamIntegration;
use crate::utils::log;
use freerdp_sys::{
    BOOL, BYTE, CHANNEL_RC_OK, IWTSListener, IWTSListenerCallback, IWTSVirtualChannel,
    IWTSVirtualChannelCallback, IWTSVirtualChannelManager, UINT,
};
use std::sync::Arc;

use super::channel::{self, ChannelCtx};
use super::pdu::write_to_channel;

// --- Control Channel (Device Enumerator) Listener ---

#[repr(C)]
pub struct ControlListenerCtx {
    pub listener_cb: IWTSListenerCallback,
    pub webcam: Arc<dyn WebcamIntegration>,
    pub channel_mgr: *mut IWTSVirtualChannelManager,
}

#[repr(C)]
pub struct ControlChannelCtx {
    pub channel_cb: IWTSVirtualChannelCallback,
    pub channel: *mut IWTSVirtualChannel,
    pub webcam: Arc<dyn WebcamIntegration>,
    pub channel_mgr: *mut IWTSVirtualChannelManager,
    pub dev_listener: Option<*mut IWTSListener>,
    pub dev_listener_ctx: Option<*mut DeviceListenerCtx>,
}

pub unsafe extern "C" fn on_new_control_channel(
    listener_cb: *mut IWTSListenerCallback,
    p_channel: *mut IWTSVirtualChannel,
    _data: *mut BYTE,
    pb_accept: *mut BOOL,
    pp_callback: *mut *mut IWTSVirtualChannelCallback,
) -> UINT {
    let lctx = listener_cb as *mut ControlListenerCtx;
    let webcam = unsafe { (*lctx).webcam.clone() };
    let channel_mgr = unsafe { (*lctx).channel_mgr };

    log::info!("Webcam Control: OnNewChannelConnection — channel={p_channel:?}, accepting");

    unsafe {
        *pb_accept = true.into();
    }

    let mut channel_ctx = Box::new(ControlChannelCtx {
        channel_cb: IWTSVirtualChannelCallback {
            OnDataReceived: Some(on_control_data),
            OnOpen: Some(on_control_open),
            OnClose: Some(on_control_close),
            ..unsafe { std::mem::zeroed() }
        },
        channel: p_channel,
        webcam,
        channel_mgr,
        dev_listener: None,
        dev_listener_ctx: None,
    });

    unsafe {
        *pp_callback = &mut channel_ctx.channel_cb;
    }

    let _ = Box::into_raw(channel_ctx);
    CHANNEL_RC_OK
}

pub unsafe extern "C" fn on_control_open(cb: *mut IWTSVirtualChannelCallback) -> UINT {
    let ctx = cb as *mut ControlChannelCtx;
    log::info!("Webcam Control: Channel opened. Sending SelectVersionRequest...");

    // SelectVersionRequest: version = 1, msg_id = 3
    let pdu = &[1u8, 3u8];
    unsafe { write_to_channel((*ctx).channel, pdu) };
    CHANNEL_RC_OK
}

pub unsafe extern "C" fn on_control_data(
    cb: *mut IWTSVirtualChannelCallback,
    stream: *mut freerdp_sys::wStream,
) -> UINT {
    let ctx = unsafe { &mut *(cb as *mut ControlChannelCtx) };
    if stream.is_null() {
        return CHANNEL_RC_OK;
    }

    let s = unsafe { &*stream };
    let bytes = unsafe { std::slice::from_raw_parts(s.pointer, s.length) };

    if bytes.len() < 2 {
        log::error!("Webcam Control PDU too short: {}", bytes.len());
        return CHANNEL_RC_OK;
    }
    let version = bytes[0];
    let msg_id = bytes[1];
    log::info!(
        "Webcam Control PDU received: version={version} msg_id={msg_id} len={}",
        bytes.len()
    );

    if msg_id == 0x04 {
        // CAM_MSG_ID_SelectVersionResponse
        log::info!("Webcam Control: Version response received. Sending DeviceAddedNotification...");
        // Send DeviceAddedNotification: version = 1, msg_id = 5
        let mut pdu = Vec::new();
        pdu.push(1u8); // version
        pdu.push(5u8); // msg_id (CAM_MSG_ID_DeviceAddedNotification)

        // DeviceName (UTF-16 LE null-terminated): e.g. "UDS Camera\0" or "Mock Camera\0"
        let name = ctx.webcam.get_device_name();
        let mut utf16: Vec<u16> = name.encode_utf16().collect();
        utf16.push(0); // Null terminator
        for &val in &utf16 {
            pdu.extend_from_slice(&val.to_le_bytes());
        }

        // VirtualChannelName (ASCII null-terminated): "RDCamera_Capture_Device_0\0"
        let channel_name = "RDCamera_Capture_Device_0\0";
        pdu.extend_from_slice(channel_name.as_bytes());

        unsafe { write_to_channel(ctx.channel, &pdu) };

        log::info!("Webcam Control: Creating dynamic listener for RDCamera_Capture_Device_0...");
        let (raw_dev_ctx, dev_listener, err) = unsafe {
            create_device_listener(
                ctx.webcam.clone(),
                ctx.channel_mgr,
                "RDCamera_Capture_Device_0",
            )
        };
        if err == CHANNEL_RC_OK {
            ctx.dev_listener = Some(dev_listener);
            ctx.dev_listener_ctx = Some(raw_dev_ctx);
            log::info!("Webcam Control: Dynamic listener created successfully");
        } else {
            log::error!("Webcam Control: Failed to create dynamic listener: {err}");
        }
    }

    CHANNEL_RC_OK
}

pub unsafe extern "C" fn on_control_close(cb: *mut IWTSVirtualChannelCallback) -> UINT {
    let ctx = cb as *mut ControlChannelCtx;
    log::info!("Webcam Control: Channel closed. Cleaning up listeners...");

    unsafe {
        if let Some(l) = (*ctx).dev_listener.take() {
            let mgr = (*ctx).channel_mgr;
            if let Some(destroy_fn) = (*mgr).DestroyListener {
                destroy_fn(mgr, l);
            }
        }
        if let Some(c) = (*ctx).dev_listener_ctx.take() {
            let _ = Box::from_raw(c);
        }
        let _ = Box::from_raw(ctx);
    }
    CHANNEL_RC_OK
}

// --- Device Channel Listener ---

#[repr(C)]
pub struct DeviceListenerCtx {
    pub listener_cb: IWTSListenerCallback,
    pub webcam: Arc<dyn WebcamIntegration>,
}

pub unsafe extern "C" fn on_new_device_channel(
    listener_cb: *mut IWTSListenerCallback,
    p_channel: *mut IWTSVirtualChannel,
    _data: *mut BYTE,
    pb_accept: *mut BOOL,
    pp_callback: *mut *mut IWTSVirtualChannelCallback,
) -> UINT {
    let lctx = listener_cb as *mut DeviceListenerCtx;
    let webcam = unsafe { (*lctx).webcam.clone() };

    log::info!("Webcam Device: OnNewChannelConnection — channel={p_channel:?}, accepting");

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

    unsafe {
        *pp_callback = &mut channel_ctx.channel_cb;
    }

    let _ = Box::into_raw(channel_ctx);
    CHANNEL_RC_OK
}

// --- Public Initialization Functions ---

pub(super) fn create_listener(
    webcam: Arc<dyn WebcamIntegration>,
    channel_mgr: *mut IWTSVirtualChannelManager,
) -> (*mut ControlListenerCtx, *mut IWTSListener, UINT) {
    let mut listener_ctx = Box::new(ControlListenerCtx {
        listener_cb: IWTSListenerCallback {
            OnNewChannelConnection: Some(on_new_control_channel),
            pInterface: std::ptr::null_mut(), // unused
        },
        webcam,
        channel_mgr,
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

pub(super) unsafe fn create_device_listener(
    webcam: Arc<dyn WebcamIntegration>,
    channel_mgr: *mut IWTSVirtualChannelManager,
    device_id: &str,
) -> (*mut DeviceListenerCtx, *mut IWTSListener, UINT) {
    let mut listener_ctx = Box::new(DeviceListenerCtx {
        listener_cb: IWTSListenerCallback {
            OnNewChannelConnection: Some(on_new_device_channel),
            pInterface: std::ptr::null_mut(),
        },
        webcam,
    });

    let mut listener_handle: *mut IWTSListener = std::ptr::null_mut();
    let c_device_id = std::ffi::CString::new(device_id).unwrap();
    let error = unsafe {
        (*channel_mgr).CreateListener.unwrap_unchecked()(
            channel_mgr,
            c_device_id.as_ptr(),
            0,
            &mut listener_ctx.listener_cb,
            &mut listener_handle,
        )
    };

    let raw_ctx = Box::into_raw(listener_ctx);
    (raw_ctx, listener_handle, error)
}
