// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
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
//
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

#![allow(dead_code)]
use std::sync::Arc;

use freerdp_sys::{
    AUDIO_FORMAT, BOOL, BYTE, CHANNEL_RC_NO_MEMORY, CHANNEL_RC_OK,
    PFREERDP_RDPSND_DEVICE_ENTRY_POINTS, UINT, UINT32, WAVE_FORMAT_PCM, freerdp_rdpsnd_get_context,
    rdpsndDevicePlugin,
};

use crate::context::OwnerFromCtx;
use crate::integrations::AudioOutputIntegration;
use crate::utils::log;

#[repr(C)]
pub struct SoundPlugin {
    device: rdpsndDevicePlugin,

    // Custom data
    audio: Option<Arc<dyn AudioOutputIntegration>>,
}

// Returns CHANNEL_RC_OK on success, or an error code on failure. (it's marked as BOOL on freerdp lib, but ist's actually a UINT32)
// Note that rdpsnd devices has a different entry point signature than other channels adding. This one is the correct one for rdpsnd.
// and will need casting when used on the addin provider.
pub unsafe extern "C" fn sound_entry(p_entry_points: PFREERDP_RDPSND_DEVICE_ENTRY_POINTS) -> UINT {
    // Should never
    if p_entry_points.is_null() {
        return CHANNEL_RC_NO_MEMORY;
    }

    let mut plugin = Box::new(SoundPlugin {
        device: rdpsndDevicePlugin {
            Open: Some(open),
            FormatSupported: Some(format_supported),
            GetVolume: Some(get_volume),
            SetVolume: Some(set_volume),
            Play: Some(play),
            Close: Some(close),
            Free: Some(free),
            // inicializa otros campos si los hay
            ..unsafe { std::mem::zeroed() }
        },
        audio: None,
    });

    unsafe {
        if let Some(register_fnc) = (*p_entry_points).pRegisterRdpsndDevice {
            register_fnc(
                (*p_entry_points).rdpsnd,
                &mut plugin.device as *mut rdpsndDevicePlugin,
            );
        }
    }

    // Ensure not to be dropped
    _ = Box::into_raw(plugin);

    log::debug!(
        "Sound addin entry called with entry points: {:?}",
        p_entry_points
    );
    // Here we would initialize the sound channel using the provided entry points.
    CHANNEL_RC_OK
}

unsafe extern "C" fn open(
    device: *mut rdpsndDevicePlugin,
    format: *const AUDIO_FORMAT,
    latency: UINT32,
) -> BOOL {
    log::debug!(
        "Sound device open called with format: {:?}, latency: {}",
        format,
        latency
    );
    let plugin = unsafe { &mut *(device as *mut SoundPlugin) };

    if let Some(rdp) = (unsafe { freerdp_rdpsnd_get_context(plugin.device.rdpsnd) }).owner() {
        let latency_threshold = rdp.config.settings.redirections.sound_latency_threshold;

        if let Some(audio_integration) = &rdp.config.integrations.audio_output {
            if let Some(audio_handle) = plugin.audio.take() {
                log::debug!("Sound device already opened, closing existing audio handle.");
                audio_handle.close();
            }

            audio_integration.open(
                unsafe { (*format).nChannels },
                unsafe { (*format).nSamplesPerSec },
                unsafe { (*format).wBitsPerSample },
                latency_threshold.map(|v| v as u32),
            );
            plugin.audio = Some(audio_integration.clone());
            true.into()
        } else {
            log::warn!("Audio output integration not configured.");
            false.into()
        }
    } else {
        log::error!("Failed to obtain Rdp owner context in sound open.");
        false.into()
    }
}

unsafe extern "C" fn format_supported(
    _device: *mut rdpsndDevicePlugin,
    format: *const AUDIO_FORMAT,
) -> BOOL {
    match unsafe { (*format).wFormatTag } {
        x if x == WAVE_FORMAT_PCM as u16 => {
            if unsafe { (*format).cbSize == 0 }
                && (unsafe { (*format).nSamplesPerSec } <= 48000)
                && (unsafe { (*format).wBitsPerSample } == 8
                    || unsafe { (*format).wBitsPerSample } == 16
                    || unsafe { (*format).wBitsPerSample } == 24
                    || unsafe { (*format).wBitsPerSample } == 32)
                && (unsafe { (*format).nChannels } >= 1 && unsafe { (*format).nChannels } <= 2)
            {
                return true.into();
            }
            false.into()
        }
        _ => false.into(),
    }
}

unsafe extern "C" fn get_volume(device: *mut rdpsndDevicePlugin) -> UINT32 {
    let plugin = unsafe { &*(device as *mut SoundPlugin) };
    if let Some(audio) = &plugin.audio {
        audio.get_volume()
    } else {
        0
    }
}

unsafe extern "C" fn set_volume(device: *mut rdpsndDevicePlugin, volume: UINT32) -> BOOL {
    let plugin = unsafe { &mut *(device as *mut SoundPlugin) };
    if let Some(audio) = &plugin.audio {
        audio.set_volume(volume);
    }
    true.into()
}

unsafe extern "C" fn play(
    _device: *mut rdpsndDevicePlugin,
    data: *const BYTE,
    size: usize,
) -> UINT {
    let plugin = unsafe { &mut *(_device as *mut SoundPlugin) };
    if let Some(audio) = &plugin.audio {
        let slice = unsafe { std::slice::from_raw_parts(data, size) };
        audio.play(slice)
    } else {
        log::error!("Audio handle not initialized in play.");
        0 // No latency if audio not initialized
    }
}

unsafe extern "C" fn close(_device: *mut rdpsndDevicePlugin) {
    log::debug!("Sound device close called.");
    let plugin = unsafe { &mut *(_device as *mut SoundPlugin) };
    if let Some(audio) = plugin.audio.take() {
        audio.close();
    }
}

unsafe extern "C" fn free(device: *mut rdpsndDevicePlugin) {
    log::debug!("Sound device free called.");
    let _plugin = unsafe { Box::from_raw(device as *mut SoundPlugin) };
    // The Box will be dropped here, freeing the memory
}
