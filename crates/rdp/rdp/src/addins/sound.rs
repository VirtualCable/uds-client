#![allow(dead_code)]
use freerdp_sys::{
    AUDIO_FORMAT, BOOL, BYTE, CHANNEL_RC_NO_MEMORY, CHANNEL_RC_OK,
    PFREERDP_RDPSND_DEVICE_ENTRY_POINTS, UINT, UINT32, rdpsndDevicePlugin,
};

use shared::log;

pub struct SoundPlugin {
    device: rdpsndDevicePlugin,

    // Custom data
    pub volume: u32,
}

// Returns CHANNEL_RC_OK on success, or an error code on failure. (it's marked as BOOL on freerdp lib, but ist's actually a UINT32)
// Note that rdpsnd devices has a different entry point signature than other channels adding. This one is the correct one for rdpsnd.
// and will need casting when used on the addin provider.
pub unsafe extern "C" fn sound_entry(
    p_entry_points: PFREERDP_RDPSND_DEVICE_ENTRY_POINTS,
) -> UINT {
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
        volume: 0xFFFFFFFF,
    });

    // let args: *const ADDIN_ARGV = unsafe { (*p_entry_points).args };
    // mayby use args to configure the plugin

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
    _device: *mut rdpsndDevicePlugin,
    format: *const AUDIO_FORMAT,
    latency: UINT32,
) -> BOOL {
    log::debug!(
        "Sound device open called with format: {:?}, latency: {}",
        format,
        latency
    );
    true.into()
}

unsafe extern "C" fn format_supported(
    _device: *mut rdpsndDevicePlugin,
    format: *const AUDIO_FORMAT,
) -> BOOL {
    log::debug!(
        "Sound device format_supported called with format: {:?}",
        unsafe { *format }
    );
    true.into()
}

unsafe extern "C" fn get_volume(device: *mut rdpsndDevicePlugin) -> UINT32 {
    let plugin = unsafe { &*(device as *mut SoundPlugin) };
    log::debug!(
        "Sound device get_volume called, current volume: {}",
        plugin.volume
    );
    plugin.volume
}

unsafe extern "C" fn set_volume(device: *mut rdpsndDevicePlugin, volume: UINT32) -> BOOL {
    let plugin = unsafe { &mut *(device as *mut SoundPlugin) };
    log::debug!("Sound device set_volume called, new volume: {}", volume);
    plugin.volume = volume;
    true.into()
}

unsafe extern "C" fn play(
    _device: *mut rdpsndDevicePlugin,
    data: *const BYTE,
    size: usize,
) -> UINT {
    log::debug!(
        "Sound device play called with data pointer: {:?}, size: {}",
        data,
        size
    );
    CHANNEL_RC_OK as UINT
}

unsafe extern "C" fn close(_device: *mut rdpsndDevicePlugin) {
    log::debug!("Sound device close called.");
}

unsafe extern "C" fn free(device: *mut rdpsndDevicePlugin) {
    log::debug!("Sound device free called.");
    let _plugin = unsafe { Box::from_raw(device as *mut SoundPlugin) };
    // The Box will be dropped here, freeing the memory
}
