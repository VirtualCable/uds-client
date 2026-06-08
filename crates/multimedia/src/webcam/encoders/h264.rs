// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

use std::ptr;
use crate::webcam::encoders::VideoEncoder;
use crate::webcam::openh264::{
    self, ISVCEncoder, SEncParamBase, SSourcePicture, SFrameBSInfo,
};
use shared::log;

pub struct H264Encoder {
    encoder: *mut ISVCEncoder,
    width: u32,
    height: u32,
    fps: u32,
    y_plane: Vec<u8>,
    u_plane: Vec<u8>,
    v_plane: Vec<u8>,
    frame_index: u64,
}

// Ensure the encoder can be safely moved across thread boundaries
unsafe impl Send for H264Encoder {}

impl H264Encoder {
    pub fn new() -> Result<Self, String> {
        let encoder = openh264::create_encoder()?;
        Ok(H264Encoder {
            encoder,
            width: 0,
            height: 0,
            fps: 0,
            y_plane: Vec::new(),
            u_plane: Vec::new(),
            v_plane: Vec::new(),
            frame_index: 0,
        })
    }
}

impl Drop for H264Encoder {
    fn drop(&mut self) {
        if !self.encoder.is_null() {
            unsafe {
                let uninit_fn = (*(*self.encoder).vtbl).uninitialize;
                let _ = uninit_fn(self.encoder);
                openh264::destroy_encoder(self.encoder);
            }
            self.encoder = ptr::null_mut();
        }
    }
}

impl VideoEncoder for H264Encoder {
    fn init(&mut self, width: u32, height: u32, fps: u32, _quality: u32) -> Result<(), String> {
        if self.encoder.is_null() {
            return Err("Encoder is not created".to_string());
        }

        self.width = width;
        self.height = height;
        self.fps = fps;
        self.frame_index = 0;

        // Allocate YUV420P planes:
        // Y: width * height
        // U: (width/2) * (height/2)
        // V: (width/2) * (height/2)
        let y_len = (width * height) as usize;
        let uv_len = ((width / 2) * (height / 2)) as usize;
        self.y_plane = vec![0u8; y_len];
        self.u_plane = vec![128u8; uv_len];
        self.v_plane = vec![128u8; uv_len];

        // Prepare configuration base parameters
        // target bitrate calculation based on width * height * fps
        let target_bitrate = (width * height * fps * 2 / 10) as i32; // basic estimation

        let params = SEncParamBase {
            f_usage_type: 0, // CAMERA_VIDEO_REAL_TIME
            i_pic_width: width as i32,
            i_pic_height: height as i32,
            i_target_bitrate: target_bitrate,
            i_rc_mode: -1, // RC_OFF_MODE
            f_max_frame_rate: fps as f32,
        };

        unsafe {
            let init_fn = (*(*self.encoder).vtbl).initialize;
            let ret = init_fn(self.encoder, &params);
            if ret != 0 {
                return Err(format!("OpenH264 initialize failed with error code: {ret}"));
            }

            let set_option_fn = (*(*self.encoder).vtbl).set_option;

            // ENCODER_OPTION_DATAFORMAT = 0, videoFormatI420 = 23
            let mut video_format = 23i32;
            let _ret_format = set_option_fn(self.encoder, 0, &mut video_format as *mut i32 as *mut std::ffi::c_void);

            // ENCODER_OPTION_TRACE_LEVEL = 19
            let mut log_level = 2i32;
            let _ret_level = set_option_fn(self.encoder, 19, &mut log_level as *mut i32 as *mut std::ffi::c_void);

            // ENCODER_OPTION_TRACE_CALLBACK = 20
            let mut callback: openh264::WelsTraceCallback = openh264_trace_callback;
            let _ret_callback = set_option_fn(self.encoder, 20, &mut callback as *mut _ as *mut std::ffi::c_void);
        }

        log::info!("OpenH264 initialized successfully: {}x{} @ {}fps", width, height, fps);
        Ok(())
    }

    fn encode(&mut self, rgb: &[u8]) -> Result<Vec<u8>, String> {
        if self.encoder.is_null() {
            return Err("Encoder is not initialized".to_string());
        }

        let width = self.width as usize;
        let height = self.height as usize;

        if rgb.len() < width * height * 3 {
            return Err("RGB buffer is too small for the configured dimensions".to_string());
        }

        // Perform fast, cache-friendly integer-based RGB24 to YUV420P (I420) conversion.
        // Bit shifts (>> 8) are used to perform division by 256 for integer arithmetic.
        let y_stride = width;
        let uv_stride = width / 2;

        for j in 0..height {
            let y_row_offset = j * y_stride;
            let uv_row_offset = (j / 2) * uv_stride;
            let rgb_row_offset = j * width * 3;

            for i in 0..width {
                let rgb_idx = rgb_row_offset + i * 3;
                let r = rgb[rgb_idx] as i32;
                let g = rgb[rgb_idx + 1] as i32;
                let b = rgb[rgb_idx + 2] as i32;

                // Y plane
                let y_val = ((66 * r + 129 * g + 25 * b + 128) >> 8) + 16;
                self.y_plane[y_row_offset + i] = y_val.clamp(0, 255) as u8;

                // U & V planes (subsampled 2x2)
                if j % 2 == 0 && i % 2 == 0 {
                    let u_val = ((-38 * r - 74 * g + 112 * b + 128) >> 8) + 128;
                    let v_val = ((112 * r - 94 * g - 18 * b + 128) >> 8) + 128;
                    self.u_plane[uv_row_offset + (i / 2)] = u_val.clamp(0, 255) as u8;
                    self.v_plane[uv_row_offset + (i / 2)] = v_val.clamp(0, 255) as u8;
                }
            }
        }

        let timestamp_ms = (self.frame_index * 1000) / self.fps.max(1) as u64;
        self.frame_index += 1;

        // Configure SSourcePicture
        let src_pic = SSourcePicture {
            i_color_format: 23, // videoFormatI420
            i_stride: [y_stride as i32, uv_stride as i32, uv_stride as i32, 0],
            p_data: [
                self.y_plane.as_mut_ptr(),
                self.u_plane.as_mut_ptr(),
                self.v_plane.as_mut_ptr(),
                ptr::null_mut(),
            ],
            i_pic_width: self.width as i32,
            i_pic_height: self.height as i32,
            ui_time_stamp: timestamp_ms as libc::c_longlong,
        };

        let mut bs_info = SFrameBSInfo {
            i_layer_num: 0,
            s_layer_info: unsafe { std::mem::zeroed() },
            e_frame_type: 0,
            i_frame_size_in_bytes: 0,
            ui_time_stamp: 0,
        };

        unsafe {
            let encode_fn = (*(*self.encoder).vtbl).encode_frame;
            let ret = encode_fn(self.encoder, &src_pic, &mut bs_info);
            if ret != 0 {
                return Err(format!("OpenH264 encode_frame failed with code: {ret}"));
            }

            // Calculate total size from layers
            let mut total_size = 0;
            for layer_idx in 0..bs_info.i_layer_num as usize {
                let layer = &bs_info.s_layer_info[layer_idx];
                for nal_idx in 0..layer.i_nal_count as usize {
                    total_size += *layer.p_nal_length_in_byte.add(nal_idx) as usize;
                }
            }

            if total_size == 0 {
                return Ok(Vec::new());
            }

            // Gather all layer bitstream buffers
            let mut encoded_data = Vec::with_capacity(total_size);
            for layer_idx in 0..bs_info.i_layer_num as usize {
                let layer = &bs_info.s_layer_info[layer_idx];
                let mut layer_size = 0;
                for nal_idx in 0..layer.i_nal_count as usize {
                    let nal_len = *layer.p_nal_length_in_byte.add(nal_idx) as usize;
                    layer_size += nal_len;
                }
                if layer_size > 0 && !layer.p_bs_buf.is_null() {
                    let slice = std::slice::from_raw_parts(layer.p_bs_buf, layer_size);
                    encoded_data.extend_from_slice(slice);
                }
            }

            let filtered_data = filter_annex_b_nal_units(&encoded_data);
            Ok(filtered_data)
        }
    }
}

fn filter_annex_b_nal_units(data: &[u8]) -> Vec<u8> {
    let mut filtered = Vec::with_capacity(data.len());
    let mut i = 0;
    let len = data.len();
    let mut starts = Vec::new();

    // Scan for all 3-byte and 4-byte start codes
    while i < len {
        if i + 4 <= len && &data[i..i+4] == &[0, 0, 0, 1] {
            starts.push((i, 4));
            i += 4;
        } else if i + 3 <= len && &data[i..i+3] == &[0, 0, 1] {
            starts.push((i, 3));
            i += 3;
        } else {
            i += 1;
        }
    }

    for idx in 0..starts.len() {
        let (start_pos, code_len) = starts[idx];
        let payload_start = start_pos + code_len;
        let payload_end = if idx + 1 < starts.len() {
            starts[idx + 1].0
        } else {
            len
        };

        if payload_start < payload_end {
            let header_byte = data[payload_start];
            let nal_type = header_byte & 0x1F;
            if nal_type != 9 {
                filtered.extend_from_slice(&[0, 0, 0, 1]);
                filtered.extend_from_slice(&data[payload_start..payload_end]);
            }
        }
    }
    filtered
}

unsafe extern "C" fn openh264_trace_callback(
    _context: *mut std::ffi::c_void,
    level: libc::c_int,
    message: *const libc::c_char,
) {
    if message.is_null() {
        return;
    }
    let msg = unsafe { std::ffi::CStr::from_ptr(message) }.to_string_lossy();
    let msg_trimmed = msg.trim();
    if msg_trimmed.is_empty() {
        return;
    }

    // Ignore the profile setting warning to keep logs clean
    if msg_trimmed.contains("doesn't support profile") {
        return;
    }

    match level {
        1 => log::error!("OpenH264: {}", msg_trimmed), // WELS_LOG_ERROR
        2 => log::warn!("OpenH264: {}", msg_trimmed),  // WELS_LOG_WARNING
        4 => log::info!("OpenH264: {}", msg_trimmed),  // WELS_LOG_INFO
        _ => log::debug!("OpenH264: {}", msg_trimmed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_annex_b_nal_units() {
        // Prepare a mock Annex B bitstream containing:
        // 1. NAL Unit type 7 (SPS) with start code 0x000001
        // 2. NAL Unit type 9 (AUD) with start code 0x00000001
        // 3. NAL Unit type 5 (IDR) with start code 0x00000001
        let mock_data = vec![
            0, 0, 1, 7, 10, 11, 12, // SPS
            0, 0, 0, 1, 9, 20, 21,  // AUD (to be filtered out)
            0, 0, 0, 1, 5, 30, 31,  // IDR
        ];
        
        let expected = vec![
            0, 0, 0, 1, 7, 10, 11, 12, // SPS
            0, 0, 0, 1, 5, 30, 31,     // IDR
        ];
        
        let filtered = filter_annex_b_nal_units(&mock_data);
        assert_eq!(filtered, expected);
    }
}
