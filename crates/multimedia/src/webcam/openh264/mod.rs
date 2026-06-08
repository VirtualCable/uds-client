// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use std::path::PathBuf;
use shared::log;

pub static OPENH264_AVAILABLE: AtomicBool = AtomicBool::new(false);
static INIT: Once = Once::new();

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SEncParamBase {
    pub f_usage_type: libc::c_int,
    pub i_pic_width: libc::c_int,
    pub i_pic_height: libc::c_int,
    pub i_target_bitrate: libc::c_int,
    pub i_rc_mode: libc::c_int,
    pub f_max_frame_rate: f32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SSourcePicture {
    pub i_color_format: libc::c_int,
    pub i_stride: [libc::c_int; 4],
    pub p_data: [*mut u8; 4],
    pub i_pic_width: libc::c_int,
    pub i_pic_height: libc::c_int,
    pub ui_time_stamp: libc::c_longlong,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SLayerBSInfo {
    pub ui_temporal_id: u8,
    pub ui_spatial_id: u8,
    pub ui_quality_id: u8,
    pub e_frame_type: libc::c_int,
    pub ui_layer_type: u8,
    pub i_sub_seq_id: libc::c_int,
    pub i_nal_count: libc::c_int,
    pub p_nal_length_in_byte: *mut libc::c_int,
    pub p_bs_buf: *mut u8,
    pub r_psnr: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SFrameBSInfo {
    pub i_layer_num: libc::c_int,
    pub s_layer_info: [SLayerBSInfo; 128],
    pub e_frame_type: libc::c_int,
    pub i_frame_size_in_bytes: libc::c_int,
    pub ui_time_stamp: libc::c_longlong,
}

#[repr(C)]
pub struct ISVCEncoder {
    pub vtbl: *const ISVCEncoderVtbl,
}

#[repr(C)]
pub struct ISVCEncoderVtbl {
    pub initialize: unsafe extern "C" fn(*mut ISVCEncoder, *const SEncParamBase) -> libc::c_int,
    pub initialize_ext: unsafe extern "C" fn(*mut ISVCEncoder, *const c_void) -> libc::c_int,
    pub get_default_params: unsafe extern "C" fn(*mut ISVCEncoder, *mut c_void) -> libc::c_int,
    pub uninitialize: unsafe extern "C" fn(*mut ISVCEncoder) -> libc::c_int,
    pub encode_frame: unsafe extern "C" fn(*mut ISVCEncoder, *const SSourcePicture, *mut SFrameBSInfo) -> libc::c_int,
    pub encode_parameter_sets: unsafe extern "C" fn(*mut ISVCEncoder, *mut SFrameBSInfo) -> libc::c_int,
    pub force_intra_frame: unsafe extern "C" fn(*mut ISVCEncoder, bool) -> libc::c_int,
    pub set_option: unsafe extern "C" fn(*mut ISVCEncoder, libc::c_int, *mut c_void) -> libc::c_int,
    pub get_option: unsafe extern "C" fn(*mut ISVCEncoder, libc::c_int, *mut c_void) -> libc::c_int,
}

pub type WelsTraceCallback = unsafe extern "C" fn(context: *mut c_void, level: libc::c_int, message: *const libc::c_char);

// Function types we need to load from the DLL/so/dylib
type WelsCreateSVCEncoderFn = unsafe extern "C" fn(pp_encoder: *mut *mut ISVCEncoder) -> libc::c_int;
type WelsDestroySVCEncoderFn = unsafe extern "C" fn(p_encoder: *mut ISVCEncoder);

// Keep the loaded library in memory once loaded
static mut LIB_HANDLE: Option<libloading::Library> = None;
static mut CREATE_ENCODER_FN: Option<WelsCreateSVCEncoderFn> = None;
static mut DESTROY_ENCODER_FN: Option<WelsDestroySVCEncoderFn> = None;

pub fn h264_available() -> bool {
    INIT.call_once(|| {
        match init_openh264_library() {
            Ok(_) => {
                log::info!("OpenH264 library loaded successfully.");
                OPENH264_AVAILABLE.store(true, Ordering::Relaxed);
            }
            Err(e) => {
                log::warn!("OpenH264 library failed to load (will fallback to MJPEG): {e}");
                OPENH264_AVAILABLE.store(false, Ordering::Relaxed);
            }
        }
    });
    OPENH264_AVAILABLE.load(Ordering::Relaxed)
}

fn get_executable_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

fn init_openh264_library() -> Result<(), String> {
    let mut possible_paths = Vec::new();

    // 1. Search in application data directories and executable dir
    if let Some(exe_dir) = get_executable_dir() {
        #[cfg(target_os = "windows")]
        {
            possible_paths.push(exe_dir.join("openh264.dll"));
            possible_paths.push(exe_dir.join("openh264-2.6.0-win64.dll"));
            possible_paths.push(exe_dir.join("openh264-2.5.1-win64.dll"));
        }
        #[cfg(target_os = "macos")]
        {
            possible_paths.push(exe_dir.join("libopenh264.dylib"));
        }
        #[cfg(target_os = "linux")]
        {
            possible_paths.push(exe_dir.join("libopenh264.so"));
        }
    }

    // 2. Platform-specific paths
    #[cfg(target_os = "macos")]
    {
        possible_paths.push(PathBuf::from("/Library/Application Support/UDSClient/openh264/libopenh264.dylib"));
    }

    // 3. Fallback to system library paths (handled by libloading itself if we pass a simple name)
    #[cfg(target_os = "windows")]
    let fallback_names = &["openh264.dll"];
    #[cfg(target_os = "macos")]
    let fallback_names = &["libopenh264.dylib"];
    #[cfg(target_os = "linux")]
    let fallback_names = &["libopenh264.so", "libopenh264.so.8", "libopenh264.so.7"];

    // Try specific paths first
    let mut loaded_lib = None;
    for path in possible_paths {
        if path.exists() {
            log::info!("Trying to load OpenH264 from: {:?}", path);
            match unsafe { libloading::Library::new(&path) } {
                Ok(lib) => {
                    loaded_lib = Some(lib);
                    break;
                }
                Err(e) => {
                    log::warn!("Failed to load from {:?}: {}", path, e);
                }
            }
        }
    }

    // If not found in specific paths, try loading from system paths using fallbacks
    let lib = if let Some(lib) = loaded_lib {
        lib
    } else {
        let mut loaded = None;
        let mut last_err = None;
        for &name in fallback_names {
            log::info!("Trying to load OpenH264 from system library search path: {}", name);
            match unsafe { libloading::Library::new(name) } {
                Ok(lib) => {
                    loaded = Some(lib);
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }
        if let Some(lib) = loaded {
            lib
        } else {
            return Err(format!(
                "Could not find or load OpenH264 library in path or system: {:?}",
                last_err
            ));
        }
    };

    // Load symbols
    unsafe {
        let create_fn: libloading::Symbol<WelsCreateSVCEncoderFn> = lib.get(b"WelsCreateSVCEncoder")
            .map_err(|e| format!("Failed to find symbol WelsCreateSVCEncoder: {e}"))?;
        let destroy_fn: libloading::Symbol<WelsDestroySVCEncoderFn> = lib.get(b"WelsDestroySVCEncoder")
            .map_err(|e| format!("Failed to find symbol WelsDestroySVCEncoder: {e}"))?;

        CREATE_ENCODER_FN = Some(*create_fn);
        DESTROY_ENCODER_FN = Some(*destroy_fn);
        LIB_HANDLE = Some(lib);
    }

    Ok(())
}

pub fn create_encoder() -> Result<*mut ISVCEncoder, String> {
    if !h264_available() {
        return Err("OpenH264 library is not available".to_string());
    }

    unsafe {
        if let Some(create_fn) = CREATE_ENCODER_FN {
            let mut encoder: *mut ISVCEncoder = std::ptr::null_mut();
            let res = create_fn(&mut encoder);
            if res != 0 || encoder.is_null() {
                return Err(format!("WelsCreateSVCEncoder failed with code: {res}"));
            }
            Ok(encoder)
        } else {
            Err("WelsCreateSVCEncoder function pointer is missing".to_string())
        }
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn destroy_encoder(encoder: *mut ISVCEncoder) {
    if encoder.is_null() {
        return;
    }
    unsafe {
        if let Some(destroy_fn) = DESTROY_ENCODER_FN {
            destroy_fn(encoder);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webcam::encoders::VideoEncoder;

    #[test]
    #[ignore]
    fn test_manual_openh264_load() {
        let available = h264_available();
        println!("OpenH264 library load result: {}", available);
        assert!(available, "OpenH264 was not successfully loaded from any path!");
    }

    #[test]
    #[ignore]
    fn test_manual_openh264_encoder_init() {
        let mut encoder = crate::webcam::encoders::H264Encoder::new().expect("Failed to instantiate H264Encoder");
        let init_res = encoder.init(640, 480, 30, 80);
        println!("OpenH264 encoder initialization result: {:?}", init_res);
        assert!(init_res.is_ok(), "Failed to initialize OpenH264 encoder: {:?}", init_res);

        let dummy_rgb = vec![128u8; 640 * 480 * 3];
        for i in 0..5 {
            match encoder.encode(&dummy_rgb) {
                Ok(data) => {
                    println!("Frame {}: encoded size = {} bytes", i, data.len());
                    assert_ne!(data.len(), 0, "Frame {} generated 0 bytes!", i);
                }
                Err(e) => {
                    panic!("Frame {} failed to encode: {:?}", i, e);
                }
            }
        }
    }
}
