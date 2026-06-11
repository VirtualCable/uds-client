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
use std::time::Instant;

use freerdp_sys::{
    AUDIO_FORMAT, BOOL, BYTE, CHANNEL_RC_OK,
    IAudinDevice, AudinReceive, UINT, UINT32, WAVE_FORMAT_PCM,
    PFREERDP_AUDIN_DEVICE_ENTRY_POINTS,
};

use multimedia::audio::{MicCommand, MicHandle};
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
        return 1;
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
    unsafe {
        if (*format).wFormatTag != WAVE_FORMAT_PCM as u16 {
            return false.into();
        }
        if (*format).nChannels != 1 {
            return false.into();
        }
    }
    true.into()
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

    log::debug!("Mic device open called");

    if receive.is_none() {
        log::error!("Mic device open called with null receive callback");
        return CHANNEL_RC_OK;
    }

    let receive_cb = receive.unwrap();
    let user_data_usize = user_data as usize;
    let fps = plugin.frames_per_packet;
    let sample_rate = plugin.format_buf.nSamplesPerSec;
    let bits = plugin.format_buf.wBitsPerSample;
    let mono_fmt = plugin.format_buf;

    let (mic, data_rx) = MicHandle::new(
        sample_rate,
        mono_fmt.nChannels,
        bits,
        fps,
    );

    plugin.mic = Some(mic);

    let st_format_tag = mono_fmt.wFormatTag;
    let st_sample_rate = mono_fmt.nSamplesPerSec;
    let st_bits_sample = mono_fmt.wBitsPerSample;
    let st_block_align = mono_fmt.nBlockAlign * 2;
    let st_avg_bps = mono_fmt.nAvgBytesPerSec * 2;

    thread::spawn(move || {
        let ud = user_data_usize as *mut std::ffi::c_void;

        let stereo_fmt = AUDIO_FORMAT {
            wFormatTag: st_format_tag,
            nChannels: 2,
            nSamplesPerSec: st_sample_rate,
            nAvgBytesPerSec: st_avg_bps,
            nBlockAlign: st_block_align,
            wBitsPerSample: st_bits_sample,
            cbSize: 0,
            data: std::ptr::null_mut(),
        };

        if sample_rate == 0 || fps == 0 {
            log::error!("Mic reader: invalid format or frames_per_packet");
            return;
        }

        let nanos = (fps as u64 * 1_000_000_000) / sample_rate as u64;
        let duration = std::time::Duration::from_nanos(nanos);
        let mono_packet_bytes = fps as usize * (bits as usize / 8);
        let stereo_packet_bytes = mono_packet_bytes * 2;

        log::info!(
            "Mic reader thread started: rate={}, frames={}, duration={:?}",
            sample_rate,
            fps,
            duration
        );

        let mut next_packet_at = Instant::now() + duration;

        loop {
            let timeout = next_packet_at.saturating_duration_since(Instant::now());

            let mono_data = match data_rx.recv_timeout(timeout) {
                Ok(data) => data,
                Err(flume::RecvTimeoutError::Timeout) => {
                    vec![0u8; mono_packet_bytes]
                }
                Err(flume::RecvTimeoutError::Disconnected) => break,
            };

            let mut stereo_data = Vec::with_capacity(stereo_packet_bytes);
            let bytes_per_sample = (bits as usize) / 8;
            for i in 0..fps as usize {
                let start = i * bytes_per_sample;
                let end = start + bytes_per_sample;
                if end <= mono_data.len() {
                    stereo_data.extend_from_slice(&mono_data[start..end]);
                    stereo_data.extend_from_slice(&mono_data[start..end]);
                } else {
                    let silence = vec![0u8; bytes_per_sample];
                    stereo_data.extend_from_slice(&silence);
                    stereo_data.extend_from_slice(&silence);
                }
            }

            unsafe {
                let res = (receive_cb)(
                    &stereo_fmt,
                    stereo_data.as_ptr() as *const BYTE,
                    stereo_data.len(),
                    ud,
                );
                if res != CHANNEL_RC_OK {
                    log::error!("Mic receive failed with {}", res);
                    break;
                }
            }

            next_packet_at += duration;
            let now = Instant::now();
            if next_packet_at <= now {
                next_packet_at = now + duration;
            }
        }
        log::info!("Mic reader thread ending");
    });

    log::debug!(
        "Mic opened: sample_rate={}, channels={}, bits={}, fps={}",
        mono_fmt.nSamplesPerSec,
        mono_fmt.nChannels,
        mono_fmt.wBitsPerSample,
        fps
    );
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
