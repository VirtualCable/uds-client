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

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
#![allow(dead_code)]
use std::thread;

use freerdp_sys::{
    AUDIO_FORMAT, BOOL, BYTE, CHANNEL_RC_NO_MEMORY, CHANNEL_RC_OK,
    IAudinDevice, AudinReceive, UINT, UINT32, WAVE_FORMAT_PCM,
    PFREERDP_AUDIN_DEVICE_ENTRY_POINTS,
};

use multimedia::mic::{MicCommand, MicHandle};
use shared::log;

#[repr(C)]
pub struct MicPlugin {
    iface: IAudinDevice,
    format_buf: AUDIO_FORMAT,
    frames_per_packet: u32,
    mic: Option<MicHandle>,
}

pub unsafe extern "C" fn mic_entry(p_entry_points: PFREERDP_AUDIN_DEVICE_ENTRY_POINTS) -> UINT {
    if p_entry_points.is_null() {
        return CHANNEL_RC_NO_MEMORY;
    }

    let mut plugin = Box::new(MicPlugin {
        iface: IAudinDevice {
            Open: Some(open),
            FormatSupported: Some(format_supported),
            SetFormat: Some(set_format),
            Close: Some(close),
            Free: Some(free),
        },
        format_buf: unsafe { std::mem::zeroed() },
        frames_per_packet: 0,
        mic: None,
    });

    unsafe {
        if let Some(register_fn) = (*p_entry_points).pRegisterAudinDevice {
            register_fn((*p_entry_points).plugin, &mut plugin.iface as *mut IAudinDevice);
        }
    }

    _ = Box::into_raw(plugin);
    log::debug!("Mic addin plugin registered");
    CHANNEL_RC_OK
}

unsafe extern "C" fn format_supported(
    _device: *mut IAudinDevice,
    format: *const AUDIO_FORMAT,
) -> BOOL {
    match unsafe { (*format).wFormatTag } {
        x if x == WAVE_FORMAT_PCM as u16 => {
            let channels = unsafe { (*format).nChannels };
            let bits = unsafe { (*format).wBitsPerSample };
            let cb_size = unsafe { (*format).cbSize };
            if cb_size == 0 && (bits == 8 || bits == 16) && (channels == 1 || channels == 2) {
                return true.into();
            }
            false.into()
        }
        _ => false.into(),
    }
}

unsafe extern "C" fn set_format(
    device: *mut IAudinDevice,
    format: *const AUDIO_FORMAT,
    frames_per_packet: UINT32,
) -> UINT {
    let plugin = unsafe { &mut *(device as *mut MicPlugin) };
    let mut fmt = unsafe { *format };
    fmt.nBlockAlign = (fmt.wBitsPerSample / 8) * fmt.nChannels;
    fmt.nAvgBytesPerSec = fmt.nSamplesPerSec * fmt.nBlockAlign as u32;
    fmt.cbSize = 0;
    fmt.data = std::ptr::null_mut();
    plugin.format_buf = fmt;
    plugin.frames_per_packet = frames_per_packet;
    log::debug!(
        "Mic set_format: sample_rate={}, channels={}, bits={}, fps={}",
        fmt.nSamplesPerSec,
        fmt.nChannels,
        fmt.wBitsPerSample,
        frames_per_packet
    );
    CHANNEL_RC_OK
}

unsafe extern "C" fn open(
    device: *mut IAudinDevice,
    receive: AudinReceive,
    user_data: *mut std::ffi::c_void,
) -> UINT {
    let plugin = unsafe { &mut *(device as *mut MicPlugin) };
    let fmt = plugin.format_buf;
    let fps = plugin.frames_per_packet;

    let (mic, data_rx) = MicHandle::new(
        fmt.nSamplesPerSec,
        fmt.nChannels,
        fmt.wBitsPerSample,
        fps,
    );

    let user_data_usize = user_data as usize;

    thread::spawn(move || {
        let fmt = AUDIO_FORMAT {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: fmt.nChannels,
            nSamplesPerSec: fmt.nSamplesPerSec,
            nAvgBytesPerSec: fmt.nAvgBytesPerSec,
            nBlockAlign: fmt.nBlockAlign,
            wBitsPerSample: fmt.wBitsPerSample,
            cbSize: 0,
            data: std::ptr::null_mut(),
        };
        let ud = user_data_usize as *mut std::ffi::c_void;
        while let Ok(pcm_data) = data_rx.recv() {
            if let Some(cb) = receive {
                let _ = unsafe { cb(&fmt, pcm_data.as_ptr() as *const BYTE, pcm_data.len(), ud) };
            }
        }
        log::debug!("Mic reader thread exiting");
    });

    plugin.mic = Some(mic);
    log::debug!("Mic opened: sample_rate={}, channels={}, bits={}, fps={}",
        fmt.nSamplesPerSec, fmt.nChannels, fmt.wBitsPerSample, fps);
    CHANNEL_RC_OK
}

unsafe extern "C" fn close(device: *mut IAudinDevice) -> UINT {
    log::debug!("Mic close called");
    let plugin = unsafe { &mut *(device as *mut MicPlugin) };
    if let Some(mic) = plugin.mic.take() {
        let _ = mic.tx.send(MicCommand::Stop);
        drop(mic);
    }
    CHANNEL_RC_OK
}

unsafe extern "C" fn free(device: *mut IAudinDevice) -> UINT {
    log::debug!("Mic free called");
    let _plugin = unsafe { Box::from_raw(device as *mut MicPlugin) };
    CHANNEL_RC_OK
}
