// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

//! OpenH264 C API type definitions and safe encoder wrapper.
//!
//! This module contains the raw FFI type definitions for the OpenH264 encoder library,
//! as well as a safe Rust wrapper (`Encoder`) that encapsulates `unsafe` operations.

use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::sync::OnceLock;

/// Base parameters for encoder initialization (matching OpenH264's SEncParamBase).
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

/// Source picture (input frame) structure (matching OpenH264's SSourcePicture).
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

/// Layer bitstream info (matching OpenH264's SLayerBSInfo).
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

/// Frame bitstream info (matching OpenH264's SFrameBSInfo).
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SFrameBSInfo {
    pub i_layer_num: libc::c_int,
    pub s_layer_info: [SLayerBSInfo; 128],
    pub e_frame_type: libc::c_int,
    pub i_frame_size_in_bytes: libc::c_int,
    pub ui_time_stamp: libc::c_longlong,
}

/// Opaque encoder handle (matching OpenH264's ISVCEncoder).
#[repr(C)]
pub struct ISVCEncoder {
    pub vtbl: *const ISVCEncoderVtbl,
}

/// Virtual function table for ISVCEncoder (matching OpenH264's ISVCEncoderVtbl).
#[repr(C)]
pub struct ISVCEncoderVtbl {
    pub initialize: unsafe extern "C" fn(*mut ISVCEncoder, *const SEncParamBase) -> libc::c_int,
    pub initialize_ext: unsafe extern "C" fn(*mut ISVCEncoder, *const c_void) -> libc::c_int,
    pub get_default_params: unsafe extern "C" fn(*mut ISVCEncoder, *mut c_void) -> libc::c_int,
    pub uninitialize: unsafe extern "C" fn(*mut ISVCEncoder) -> libc::c_int,
    pub encode_frame: unsafe extern "C" fn(
        *mut ISVCEncoder,
        *const SSourcePicture,
        *mut SFrameBSInfo,
    ) -> libc::c_int,
    pub encode_parameter_sets:
        unsafe extern "C" fn(*mut ISVCEncoder, *mut SFrameBSInfo) -> libc::c_int,
    pub force_intra_frame: unsafe extern "C" fn(*mut ISVCEncoder, bool) -> libc::c_int,
    pub set_option: unsafe extern "C" fn(*mut ISVCEncoder, libc::c_int, *mut c_void) -> libc::c_int,
    pub get_option: unsafe extern "C" fn(*mut ISVCEncoder, libc::c_int, *mut c_void) -> libc::c_int,
}

/// Trace callback type used by OpenH264 for logging.
pub type WelsTraceCallback =
    unsafe extern "C" fn(context: *mut c_void, level: libc::c_int, message: *const libc::c_char);

/// Function pointer type for WelsCreateSVCEncoder.
pub(crate) type WelsCreateSVCEncoderFn =
    unsafe extern "C" fn(pp_encoder: *mut *mut ISVCEncoder) -> libc::c_int;

/// Function pointer type for WelsDestroySVCEncoder.
pub(crate) type WelsDestroySVCEncoderFn = unsafe extern "C" fn(p_encoder: *mut ISVCEncoder);

// ---------------------------------------------------------------------------
// Safe encoder wrapper
// ---------------------------------------------------------------------------

/// Global destroy function pointer, set once during library initialization.
static DESTROY_ENCODER: OnceLock<WelsDestroySVCEncoderFn> = OnceLock::new();

/// Sets the global `WelsDestroySVCEncoder` function pointer.
///
/// Called once during library initialization in [`super::init_openh264_library`].
pub(crate) fn set_destroy_fn(fn_ptr: WelsDestroySVCEncoderFn) {
    let _ = DESTROY_ENCODER.set(fn_ptr);
}

/// Errors that can occur during encoder operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Initialization failed with the given error code.
    InitFailed(i32),
    /// Encoding failed with the given error code.
    EncodeFailed(i32),
    /// Invalid or unsupported parameter.
    ParamError,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InitFailed(code) => write!(f, "encoder initialization failed (code {code})"),
            Error::EncodeFailed(code) => write!(f, "encoding failed (code {code})"),
            Error::ParamError => write!(f, "invalid parameter"),
        }
    }
}

impl std::error::Error for Error {}

/// A safe, owning wrapper around [`ISVCEncoder`].
///
/// This type encapsulates all `unsafe` FFI calls through the vtable,
/// and automatically calls `Uninitialize` + `WelsDestroySVCEncoder` on drop.
///
/// # Panics
///
/// Constructing via [`Encoder::from_raw`] with a null pointer will panic.
#[derive(Debug)]
pub struct Encoder {
    ptr: NonNull<ISVCEncoder>,
    _marker: PhantomData<ISVCEncoder>,
}

// ---------------------------------------------------------------------------
// Safe configuration abstraction
// ---------------------------------------------------------------------------

/// Safe, ergonomic configuration for initializing the encoder.
///
/// Use the builder methods to customize parameters beyond the defaults.
///
/// # Example
///
/// ```ignore
/// let cfg = EncoderConfig::new(640, 480, 30.0)
///     .with_bitrate(2_000_000)
///     .with_rc_mode(-1); // RC_OFF_MODE
/// encoder.initialize(&cfg)?;
/// ```
#[derive(Debug, Clone)]
pub struct EncoderConfig {
    usage_type: i32,
    width: i32,
    height: i32,
    target_bitrate: i32,
    rc_mode: i32,
    max_frame_rate: f32,
}

impl EncoderConfig {
    /// Creates a new configuration with the given dimensions and frame rate.
    ///
    /// Bitrate is auto-calculated as `width * height * fps * 2 / 10`.
    /// Use [`with_bitrate`](Self::with_bitrate) to set a custom value.
    pub fn new(width: u32, height: u32, fps: f32) -> Self {
        let base_bitrate = (width * height * fps as u32 * 2 / 10) as i32;
        Self {
            usage_type: 0, // CAMERA_VIDEO_REAL_TIME
            width: width as i32,
            height: height as i32,
            target_bitrate: base_bitrate,
            rc_mode: -1, // RC_OFF_MODE
            max_frame_rate: fps,
        }
    }

    /// Sets a custom target bitrate (bps).
    pub fn with_bitrate(mut self, bitrate: i32) -> Self {
        self.target_bitrate = bitrate;
        self
    }

    /// Sets the rate control mode.
    ///
    /// Common values: `-1` = RC_OFF_MODE, `0` = RC_QUALITY_MODE,
    /// `1` = RC_BITRATE_MODE, `2` = RC_BUFFERBASED_MODE.
    pub fn with_rc_mode(mut self, mode: i32) -> Self {
        self.rc_mode = mode;
        self
    }

    /// Sets the usage type.
    ///
    /// `0` = CAMERA_VIDEO_REAL_TIME, `1` = SCREEN_CONTENT_REAL_TIME.
    pub fn with_usage_type(mut self, usage: i32) -> Self {
        self.usage_type = usage;
        self
    }

    /// Converts this config back to the raw FFI struct.
    pub(crate) fn to_base(&self) -> SEncParamBase {
        SEncParamBase {
            f_usage_type: self.usage_type,
            i_pic_width: self.width,
            i_pic_height: self.height,
            i_target_bitrate: self.target_bitrate,
            i_rc_mode: self.rc_mode,
            f_max_frame_rate: self.max_frame_rate,
        }
    }
}

// ---------------------------------------------------------------------------
// Safe encoder wrapper
// ---------------------------------------------------------------------------

// The underlying OpenH264 encoder is thread-safe.
unsafe impl Send for Encoder {}
unsafe impl Sync for Encoder {}

impl Encoder {
    /// Wraps a raw `*mut ISVCEncoder` into a safe [`Encoder`].
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer returned by `WelsCreateSVCEncoder`.
    /// The caller must ensure no other code holds a reference to the same encoder.
    pub unsafe fn from_raw(ptr: *mut ISVCEncoder) -> Self {
        Self {
            ptr: NonNull::new(ptr).expect("Encoder::from_raw received a null pointer"),
            _marker: PhantomData,
        }
    }

    /// Returns a reference to the vtable.
    fn vtbl(&self) -> &ISVCEncoderVtbl {
        // SAFETY: `self.ptr` is guaranteed non-null and valid for the lifetime of `self`.
        unsafe { &*self.ptr.as_ref().vtbl }
    }

    /// Returns a raw pointer to the underlying encoder.
    ///
    /// This is useful when the raw pointer is needed for low-level operations
    /// (e.g., passing to [`set_option`](Encoder::set_option) callbacks).
    pub fn as_ptr(&self) -> *mut ISVCEncoder {
        self.ptr.as_ptr()
    }

    /// Initializes the encoder with the given configuration.
    pub fn initialize(&mut self, cfg: &EncoderConfig) -> Result<(), Error> {
        let base = cfg.to_base();
        // SAFETY: The vtable function is valid as long as the library is loaded.
        unsafe {
            let ret = (self.vtbl().initialize)(self.ptr.as_ptr(), &base);
            if ret == 0 {
                Ok(())
            } else {
                Err(Error::InitFailed(ret))
            }
        }
    }

    /// Initializes the encoder with extended parameters.
    ///
    /// # Safety
    ///
    /// `cfg` must point to a valid `SEncParamExt` structure.
    pub unsafe fn initialize_ext(&mut self, cfg: *const c_void) -> Result<(), Error> {
        // SAFETY: The caller must ensure `cfg` points to a valid `SEncParamExt`.
        unsafe {
            let ret = (self.vtbl().initialize_ext)(self.ptr.as_ptr(), cfg);
            if ret == 0 {
                Ok(())
            } else {
                Err(Error::InitFailed(ret))
            }
        }
    }

    /// Fills `params` with default encoder parameters.
    ///
    /// # Safety
    ///
    /// `params` must point to a buffer large enough for `SEncParamExt`.
    pub unsafe fn get_default_params(&self, params: *mut c_void) -> Result<(), Error> {
        // SAFETY: The caller must ensure `params` is large enough for `SEncParamExt`.
        unsafe {
            let ret = (self.vtbl().get_default_params)(self.ptr.as_ptr() as *mut _, params);
            if ret == 0 {
                Ok(())
            } else {
                Err(Error::ParamError)
            }
        }
    }

    /// Encodes a single video frame.
    pub fn encode_frame(
        &mut self,
        src: &SSourcePicture,
        dst: &mut SFrameBSInfo,
    ) -> Result<(), Error> {
        // SAFETY: The caller must ensure `src` and `dst` are valid.
        unsafe {
            let ret = (self.vtbl().encode_frame)(self.ptr.as_ptr(), src, dst);
            if ret == 0 {
                Ok(())
            } else {
                Err(Error::EncodeFailed(ret))
            }
        }
    }

    /// Generates parameter sets (SPS/PPS) into `dst`.
    pub fn encode_parameter_sets(&mut self, dst: &mut SFrameBSInfo) -> Result<(), Error> {
        // SAFETY: The caller must ensure `dst` is valid.
        unsafe {
            let ret = (self.vtbl().encode_parameter_sets)(self.ptr.as_ptr(), dst);
            if ret == 0 {
                Ok(())
            } else {
                Err(Error::EncodeFailed(ret))
            }
        }
    }

    /// Forces the next frame to be an IDR (keyframe).
    pub fn force_intra_frame(&mut self) -> Result<(), Error> {
        // SAFETY: The vtable function is valid as long as the library is loaded.
        unsafe {
            let ret = (self.vtbl().force_intra_frame)(self.ptr.as_ptr(), true);
            if ret == 0 {
                Ok(())
            } else {
                Err(Error::EncodeFailed(ret))
            }
        }
    }

    /// Sets an encoder option.
    ///
    /// # Safety
    ///
    /// `val` must point to a value of the correct type for `opt`.
    pub unsafe fn set_option(&mut self, opt: libc::c_int, val: *mut c_void) -> Result<(), Error> {
        // SAFETY: Caller guarantees `val` is correctly typed for `opt`.
        let ret = unsafe { (self.vtbl().set_option)(self.ptr.as_ptr(), opt, val) };
        if ret == 0 {
            Ok(())
        } else {
            Err(Error::ParamError)
        }
    }

    /// Gets an encoder option.
    ///
    /// # Safety
    ///
    /// `val` must point to a buffer of the correct type for `opt`.
    pub unsafe fn get_option(&self, opt: libc::c_int, val: *mut c_void) -> Result<(), Error> {
        // SAFETY: Caller guarantees `val` is correctly typed for `opt`.
        let ret = unsafe { (self.vtbl().get_option)(self.ptr.as_ptr() as *mut _, opt, val) };
        if ret == 0 {
            Ok(())
        } else {
            Err(Error::ParamError)
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        // SAFETY: The encoder pointer is valid and the vtable is accessible.
        unsafe {
            (self.vtbl().uninitialize)(self.ptr.as_ptr());
            if let Some(destroy) = DESTROY_ENCODER.get() {
                destroy(self.ptr.as_ptr());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `Error` implements [`Display`] and [`std::error::Error`].
    #[test]
    fn test_error_display() {
        let err = Error::InitFailed(42);
        assert_eq!(err.to_string(), "encoder initialization failed (code 42)");
        assert!(std::error::Error::source(&err).is_none());

        let err = Error::EncodeFailed(-1);
        assert_eq!(err.to_string(), "encoding failed (code -1)");

        let err = Error::ParamError;
        assert_eq!(err.to_string(), "invalid parameter");
    }

    /// Verify that `Encoder` is `Send` and `Sync`.
    #[test]
    fn test_encoder_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<Encoder>();
        assert_sync::<Encoder>();
    }

    /// Verify that `from_raw` panics on null pointer.
    #[test]
    #[should_panic(expected = "null pointer")]
    fn test_from_raw_null_panics() {
        unsafe {
            let _ = Encoder::from_raw(std::ptr::null_mut());
        }
    }
}
