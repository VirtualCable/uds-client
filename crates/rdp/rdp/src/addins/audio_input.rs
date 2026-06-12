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
use std::sync::Arc;
use std::thread;

use freerdp_sys::{
    AUDIO_FORMAT, AudinReceive, BOOL, BYTE, CHANNEL_RC_OK, IAudinDevice,
    PFREERDP_AUDIN_DEVICE_ENTRY_POINTS, UINT, UINT32, WAVE_FORMAT_PCM,
};

use crate::context::OwnerFromCtx;
use crate::integrations::AudioInputIntegration;
use shared::log;

#[repr(C)]
pub struct MicPlugin {
    iface: IAudinDevice,

    // Custom data
    rdpcontext: *mut freerdp_sys::rdpContext,
    format: AUDIO_FORMAT,
    frames_per_packet: u32,
    audio_input: Option<Arc<dyn AudioInputIntegration>>,
    stop_tx: Option<flume::Sender<()>>,
}

pub unsafe extern "C" fn mic_entry(p_entry_points: PFREERDP_AUDIN_DEVICE_ENTRY_POINTS) -> UINT {
    if p_entry_points.is_null() {
        return 1;
    }

    let rdpcontext = unsafe { (*p_entry_points).rdpcontext };

    let mut plugin = Box::new(MicPlugin {
        iface: IAudinDevice {
            Open: Some(open),
            FormatSupported: Some(format_supported),
            SetFormat: Some(set_format),
            Close: Some(close),
            Free: Some(free),
        },
        rdpcontext,
        format: unsafe { std::mem::zeroed() },
        frames_per_packet: 0,
        audio_input: None,
        stop_tx: None,
    });

    unsafe {
        if let Some(register_fn) = (*p_entry_points).pRegisterAudinDevice {
            register_fn(
                (*p_entry_points).plugin,
                &mut plugin.iface as *mut IAudinDevice,
            );
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
    plugin.format = fmt;
    plugin.frames_per_packet = frames_per_packet;
    log::debug!(
        "Mic set_format: sample_rate={}, channels={}, bits={}, fps={}",
        fmt.nSamplesPerSec,
        fmt.nChannels,
        fmt.wBitsPerSample,
        frames_per_packet
    );

    if let Some(tx) = plugin
        .rdpcontext
        .owner()
        .and_then(|rdp| rdp.update_tx.as_ref())
    {
        let _ = tx.send(crate::messaging::RdpMessage::MicConfig {
            sample_rate: fmt.nSamplesPerSec,
            frames_per_packet,
        });
    }

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

    let rdp = if let Some(rdp) = plugin.rdpcontext.owner() {
        rdp
    } else {
        log::error!("Failed to obtain Rdp owner context in mic open");
        return 1;
    };

    let audio_input_integration = if let Some(ref integration) = rdp.config.integrations.audio_input
    {
        integration
    } else {
        log::warn!("Audio input integration not configured");
        return CHANNEL_RC_OK;
    };

    let receive_cb = receive.unwrap();
    let user_data_usize = user_data as usize;
    let fps = plugin.frames_per_packet;
    let sample_rate = plugin.format.nSamplesPerSec;
    let bits = plugin.format.wBitsPerSample;
    let channels = plugin.format.nChannels;

    let data_rx = match audio_input_integration.start(sample_rate, channels, bits, fps) {
        Ok(rx) => rx,
        Err(e) => {
            log::error!("Failed to start audio input integration: {}", e);
            return 1;
        }
    };

    if let Some(ref tx) = rdp.update_tx {
        let _ = tx.send(crate::messaging::RdpMessage::MicConfig {
            sample_rate,
            frames_per_packet: fps,
        });
    }

    plugin.audio_input = Some(audio_input_integration.clone());
    let (stop_tx, stop_rx) = flume::bounded(1);
    plugin.stop_tx = Some(stop_tx);

    let format_tag = plugin.format.wFormatTag;
    let sample_rate_val = plugin.format.nSamplesPerSec;
    let bits_val = plugin.format.wBitsPerSample;
    let block_align = plugin.format.nBlockAlign * 2;
    let avg_bps = plugin.format.nAvgBytesPerSec * 2;

    thread::spawn(move || {
        let ud = user_data_usize as *mut std::ffi::c_void;

        let stereo_fmt = AUDIO_FORMAT {
            wFormatTag: format_tag,
            nChannels: 2,
            nSamplesPerSec: sample_rate_val,
            nAvgBytesPerSec: avg_bps,
            nBlockAlign: block_align,
            wBitsPerSample: bits_val,
            cbSize: 0,
            data: std::ptr::null_mut(),
        };

        if sample_rate == 0 || fps == 0 {
            log::error!("Mic reader: invalid format or frames_per_packet");
            return;
        }

        let duration =
            std::time::Duration::from_nanos((fps as u64 * 1_000_000_000) / sample_rate as u64);
        let mono_packet_bytes = fps as usize * (bits as usize / 8);
        let stereo_packet_bytes = mono_packet_bytes * 2;

        log::info!(
            "Mic reader thread started: rate={}, frames={}, duration={:?}",
            sample_rate,
            fps,
            duration
        );

        while let Ok(mono_data) = data_rx.recv() {
            if stop_rx.try_recv().is_ok() {
                break;
            }

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
        }
        log::info!("Mic reader thread ending");
    });

    log::debug!(
        "Mic opened: sample_rate={}, channels={}, bits={}, fps={}",
        plugin.format.nSamplesPerSec,
        plugin.format.nChannels,
        plugin.format.wBitsPerSample,
        fps
    );
    CHANNEL_RC_OK
}

unsafe extern "C" fn close(device: *mut IAudinDevice) -> UINT {
    log::debug!("Mic close called");
    let plugin = unsafe { &mut *(device as *mut MicPlugin) };
    if let Some(stop_tx) = plugin.stop_tx.take() {
        let _ = stop_tx.send(());
    }
    if let Some(audio_input) = plugin.audio_input.take() {
        audio_input.stop();
    }
    CHANNEL_RC_OK
}

unsafe extern "C" fn free(device: *mut IAudinDevice) -> UINT {
    log::debug!("Mic free called");
    let _plugin = unsafe { Box::from_raw(device as *mut MicPlugin) };
    CHANNEL_RC_OK
}
