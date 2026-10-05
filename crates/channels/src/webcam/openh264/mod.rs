// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

pub mod types;

use anyhow::{Result, anyhow};
use shared::log;
use std::path::PathBuf;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

// Re-export public types for convenience
pub use types::{
    Encoder, EncoderConfig, ISVCEncoder, ISVCEncoderVtbl, SEncParamBase, SFrameBSInfo,
    SLayerBSInfo, SSourcePicture, WelsTraceCallback,
};

// Internal function pointer types (used for loading library symbols)
use types::{WelsCreateSVCEncoderFn, WelsDestroySVCEncoderFn, set_destroy_fn};

pub static OPENH264_AVAILABLE: AtomicBool = AtomicBool::new(false);
static INIT: Once = Once::new();

// Keep the loaded library in memory once loaded
static mut LIB_HANDLE: Option<libloading::Library> = None;
static mut CREATE_ENCODER_FN: Option<WelsCreateSVCEncoderFn> = None;

pub fn h264_available() -> bool {
    INIT.call_once(|| match init_openh264_library() {
        Ok(_) => {
            log::info!("OpenH264 library loaded successfully.");
            OPENH264_AVAILABLE.store(true, Ordering::Relaxed);
        }
        Err(e) => {
            log::warn!("OpenH264 library failed to load (will fallback to MJPEG): {e}");
            OPENH264_AVAILABLE.store(false, Ordering::Relaxed);
        }
    });
    OPENH264_AVAILABLE.load(Ordering::Relaxed)
}

fn get_executable_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
}

#[cfg(target_os = "windows")]
const LIB_NAMES: &[&str] = &[
    "openh264.dll",
    "openh264-2.6.0-win64.dll",
    "openh264-2.5.1-win64.dll",
];
// The installer stores the dylib under its soname, libopenh264.8.dylib, because that is
// the name FreeRDP's load command asks for. Renaming it there breaks FreeRDP.
#[cfg(target_os = "macos")]
const LIB_NAMES: &[&str] = &["libopenh264.8.dylib", "libopenh264.dylib"];
#[cfg(target_os = "linux")]
const LIB_NAMES: &[&str] = &["libopenh264.so", "libopenh264.so.8", "libopenh264.so.7"];

#[cfg(target_os = "macos")]
const SYSTEM_DIRS: &[&str] = &["/Library/Application Support/UDSLauncher/openh264"];
#[cfg(not(target_os = "macos"))]
const SYSTEM_DIRS: &[&str] = &[];

fn candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(exe_dir) = get_executable_dir() {
        paths.extend(LIB_NAMES.iter().map(|name| exe_dir.join(name)));
    }
    for dir in SYSTEM_DIRS {
        paths.extend(LIB_NAMES.iter().map(|name| PathBuf::from(dir).join(name)));
    }
    paths
}

fn init_openh264_library() -> Result<()> {
    let possible_paths = candidate_paths();
    let fallback_names = LIB_NAMES;

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
            log::info!(
                "Trying to load OpenH264 from system library search path: {}",
                name
            );
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
            return Err(anyhow!(
                "Could not find or load OpenH264 library in path or system: {:?}",
                last_err
            ));
        }
    };

    // Load symbols
    unsafe {
        let create_fn: libloading::Symbol<WelsCreateSVCEncoderFn> = lib
            .get(b"WelsCreateSVCEncoder")
            .map_err(|e| anyhow!("Failed to find symbol WelsCreateSVCEncoder: {e}"))?;
        let destroy_fn: libloading::Symbol<WelsDestroySVCEncoderFn> = lib
            .get(b"WelsDestroySVCEncoder")
            .map_err(|e| anyhow!("Failed to find symbol WelsDestroySVCEncoder: {e}"))?;

        CREATE_ENCODER_FN = Some(*create_fn);
        set_destroy_fn(*destroy_fn);
        LIB_HANDLE = Some(lib);
    }

    Ok(())
}

pub fn create_encoder() -> Result<Encoder> {
    if !h264_available() {
        return Err(anyhow!("OpenH264 library is not available"));
    }

    unsafe {
        if let Some(create_fn) = CREATE_ENCODER_FN {
            let mut encoder: *mut ISVCEncoder = std::ptr::null_mut();
            let res = create_fn(&mut encoder);
            if res != 0 || encoder.is_null() {
                return Err(anyhow!("WelsCreateSVCEncoder failed with code: {res}"));
            }
            Ok(Encoder::from_raw(encoder))
        } else {
            Err(anyhow!("WelsCreateSVCEncoder function pointer is missing"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webcam::encoders::VideoEncoder;

    fn candidate_file_names() -> Vec<String> {
        candidate_paths()
            .iter()
            .filter_map(|p| p.file_name()?.to_str().map(str::to_owned))
            .collect()
    }

    #[test]
    fn candidate_paths_cover_every_library_name() {
        let names = candidate_file_names();
        for expected in LIB_NAMES {
            assert!(
                names.iter().any(|name| name == expected),
                "{expected} is never searched for; found {names:?}"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_searches_the_installed_soname() {
        let installed = PathBuf::from(SYSTEM_DIRS[0]).join("libopenh264.8.dylib");
        assert!(
            candidate_paths().contains(&installed),
            "the path the installer writes is never searched: {installed:?}"
        );
    }

    #[test]
    #[ignore]
    fn test_manual_openh264_load() {
        let available = h264_available();
        println!("OpenH264 library load result: {}", available);
        assert!(
            available,
            "OpenH264 was not successfully loaded from any path!"
        );
    }

    #[test]
    #[ignore]
    fn test_manual_openh264_encoder_init() {
        let mut encoder =
            crate::webcam::encoders::H264Encoder::new().expect("Failed to instantiate H264Encoder");
        let init_res = encoder.init(640, 480, 30, 80);
        println!("OpenH264 encoder initialization result: {:?}", init_res);
        assert!(
            init_res.is_ok(),
            "Failed to initialize OpenH264 encoder: {:?}",
            init_res
        );

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
