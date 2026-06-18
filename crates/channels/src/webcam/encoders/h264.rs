// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

use crate::webcam::encoders::VideoEncoder;
use crate::webcam::openh264::{self, Encoder, EncoderConfig, SFrameBSInfo, SSourcePicture};
use anyhow::{Context, Result, bail};
use shared::log;
use std::ffi::c_void;
use std::ptr;

pub struct H264Encoder {
    encoder: Option<Encoder>,
    width: u32,
    height: u32,
    fps: u32,
    y_plane: Vec<u8>,
    u_plane: Vec<u8>,
    v_plane: Vec<u8>,
    frame_index: u64,
    has_sent_idr: bool,
    sps_pps: Vec<u8>,
}

// The underlying openh264::Encoder already implements Send+Sync.
unsafe impl Send for H264Encoder {}

impl H264Encoder {
    pub fn new() -> Result<Self> {
        let encoder = openh264::create_encoder()?;
        Ok(H264Encoder {
            encoder: Some(encoder),
            width: 0,
            height: 0,
            fps: 0,
            y_plane: Vec::new(),
            u_plane: Vec::new(),
            v_plane: Vec::new(),
            frame_index: 0,
            has_sent_idr: false,
            sps_pps: Vec::new(),
        })
    }
}

// No need for a custom Drop — `Encoder` handles uninitialize + destroy automatically.

impl VideoEncoder for H264Encoder {
    fn init(&mut self, width: u32, height: u32, fps: u32, quality: u32) -> Result<()> {
        let encoder = self.encoder.as_mut().context("Encoder is not created")?;

        self.width = width;
        self.height = height;
        self.fps = fps;
        self.frame_index = 0;
        self.has_sent_idr = false;
        self.sps_pps.clear();

        // Allocate YUV420P planes:
        // Y: width * height
        // U: (width/2) * (height/2)
        // V: (width/2) * (height/2)
        let y_len = (width * height) as usize;
        let uv_len = ((width / 2) * (height / 2)) as usize;
        self.y_plane = vec![0u8; y_len];
        self.u_plane = vec![128u8; uv_len];
        self.v_plane = vec![128u8; uv_len];

        // Prepare configuration using the safe EncoderConfig builder
        let q = if quality == 0 {
            80
        } else {
            quality.clamp(1, 100)
        };
        let base_bitrate = (width * height * fps * 2 / 10) as f64;
        let target_bitrate = (base_bitrate * (q as f64 / 100.0)) as i32;

        let config = EncoderConfig::new(width, height, fps as f32)
            .with_bitrate(target_bitrate)
            .with_rc_mode(-1); // RC_OFF_MODE

        encoder
            .initialize(&config)
            .context("OpenH264 initialize failed")?;

        // SAFETY: The option values are correctly typed for each option ID.
        unsafe {
            // ENCODER_OPTION_DATAFORMAT = 0, videoFormatI420 = 23
            let mut video_format = 23i32;
            let _ = encoder.set_option(0, &mut video_format as *mut i32 as *mut c_void);

            // ENCODER_OPTION_TRACE_LEVEL = 19
            let mut log_level = 2i32;
            let _ = encoder.set_option(19, &mut log_level as *mut i32 as *mut c_void);

            // ENCODER_OPTION_TRACE_CALLBACK = 20
            let mut callback: openh264::WelsTraceCallback = openh264_trace_callback;
            let _ = encoder.set_option(20, &mut callback as *mut _ as *mut c_void);
        }

        log::info!(
            "OpenH264 initialized successfully: {}x{} @ {}fps",
            width,
            height,
            fps
        );
        Ok(())
    }

    fn encode(&mut self, rgb: &[u8]) -> Result<Vec<u8>> {
        let encoder = self
            .encoder
            .as_mut()
            .context("Encoder is not initialized")?;

        let width = self.width as usize;
        let height = self.height as usize;

        if rgb.len() < width * height * 3 {
            bail!("RGB buffer is too small for the configured dimensions");
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

        // Force an IDR (keyframe) at the start and periodically every (fps * 2) frames
        let keyframe_interval = (self.fps * 2).max(1) as u64;
        if !self.has_sent_idr || self.frame_index.is_multiple_of(keyframe_interval) {
            let _ = encoder
                .force_intra_frame()
                .context("OpenH264 force_intra_frame failed");
        }

        encoder
            .encode_frame(&src_pic, &mut bs_info)
            .context("OpenH264 encode_frame failed")?;

        unsafe {
            // Safety: clamp FFI return values to prevent OOB on negative/bogus data.
            let num_layers = (bs_info.i_layer_num.max(0) as usize).min(128);
            let nal_cap = 1024;

            // Calculate total size from layers
            let mut total_size = 0;
            for layer_idx in 0..num_layers {
                let layer = &bs_info.s_layer_info[layer_idx];
                let num_nals = (layer.i_nal_count.max(0) as usize).min(nal_cap);
                for nal_idx in 0..num_nals {
                    if !layer.p_nal_length_in_byte.is_null() {
                        total_size += *layer.p_nal_length_in_byte.add(nal_idx) as usize;
                    }
                }
            }

            if total_size == 0 {
                return Ok(Vec::new());
            }

            // Gather all layer bitstream buffers
            let mut encoded_data = Vec::with_capacity(total_size);
            for layer_idx in 0..num_layers {
                let layer = &bs_info.s_layer_info[layer_idx];
                let num_nals = (layer.i_nal_count.max(0) as usize).min(nal_cap);
                let mut layer_size = 0;
                for nal_idx in 0..num_nals {
                    if !layer.p_nal_length_in_byte.is_null() {
                        let nal_len = *layer.p_nal_length_in_byte.add(nal_idx) as usize;
                        layer_size += nal_len;
                    }
                }
                if layer_size > 0 && !layer.p_bs_buf.is_null() {
                    let slice = std::slice::from_raw_parts(layer.p_bs_buf, layer_size);
                    encoded_data.extend_from_slice(slice);
                }
            }

            // Parse and filter Annex B NAL units
            let mut filtered_data = Vec::new();
            let mut i = 0;
            let len = encoded_data.len();
            let mut starts = Vec::new();

            // Scan for all 3-byte and 4-byte start codes
            while i < len {
                if i + 4 <= len && encoded_data[i..i + 4] == [0, 0, 0, 1] {
                    starts.push((i, 4));
                    i += 4;
                } else if i + 3 <= len && encoded_data[i..i + 3] == [0, 0, 1] {
                    starts.push((i, 3));
                    i += 3;
                } else {
                    i += 1;
                }
            }

            let mut has_idr_in_this_frame = false;

            for idx in 0..starts.len() {
                let (start_pos, code_len) = starts[idx];
                let payload_start = start_pos + code_len;
                let payload_end = if idx + 1 < starts.len() {
                    starts[idx + 1].0
                } else {
                    len
                };

                if payload_start < payload_end {
                    let header_byte = encoded_data[payload_start];
                    let nal_type = header_byte & 0x1F;
                    if self.frame_index <= 5 {
                        println!(
                            "[ENCODER] Frame index: {}, NAL unit type: {} at pos {}",
                            self.frame_index, nal_type, start_pos
                        );
                    }

                    if nal_type == 7 {
                        // SPS: Clear previous and store
                        self.sps_pps.clear();
                        self.sps_pps.extend_from_slice(&[0, 0, 0, 1]);
                        self.sps_pps
                            .extend_from_slice(&encoded_data[payload_start..payload_end]);
                    } else if nal_type == 8 {
                        // PPS: Store
                        self.sps_pps.extend_from_slice(&[0, 0, 0, 1]);
                        self.sps_pps
                            .extend_from_slice(&encoded_data[payload_start..payload_end]);
                    } else if nal_type == 5 {
                        // IDR picture
                        has_idr_in_this_frame = true;
                        if !self.sps_pps.is_empty() {
                            filtered_data.extend_from_slice(&self.sps_pps);
                        }
                        filtered_data.extend_from_slice(&[0, 0, 0, 1]);
                        filtered_data.extend_from_slice(&encoded_data[payload_start..payload_end]);
                    } else if nal_type == 1 {
                        // P-frame slice
                        if self.has_sent_idr {
                            filtered_data.extend_from_slice(&[0, 0, 0, 1]);
                            filtered_data
                                .extend_from_slice(&encoded_data[payload_start..payload_end]);
                        }
                    } else if nal_type != 9 {
                        // Other NAL types (e.g. SEI), skipping AUD (9)
                        if self.has_sent_idr {
                            filtered_data.extend_from_slice(&[0, 0, 0, 1]);
                            filtered_data
                                .extend_from_slice(&encoded_data[payload_start..payload_end]);
                        }
                    }
                }
            }

            if has_idr_in_this_frame {
                self.has_sent_idr = true;
            }

            Ok(filtered_data)
        }
    }
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
    #[ignore]
    fn test_generate_h264_dump() {
        let mut encoder = H264Encoder::new().unwrap();
        let width = 640;
        let height = 480;
        let fps = 30;
        encoder.init(width, height, fps, 3).unwrap();

        let temp_dir = std::env::var("TEMP").unwrap_or_else(|_| ".".to_string());
        let dump_path = std::path::PathBuf::from(temp_dir).join("test_stream.h264");
        let _ = std::fs::remove_file(&dump_path);

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&dump_path)
            .unwrap();

        let num_frames = 600; // 20 seconds at 30 fps
        let sq_size = 120;

        let mut total_bytes = 0;
        for f in 0..num_frames {
            let mut rgb = vec![0u8; (width * height * 3) as usize];

            // Generate a dynamic gradient background
            let b_val = ((f * 2) % 256) as u8;
            for y in 0..height {
                let y_offset = y * width * 3;
                let g_val = (y * 255 / height) as u8;
                for x in 0..width {
                    let r_val = (x * 255 / width) as u8;
                    let idx = (y_offset + x * 3) as usize;
                    rgb[idx] = r_val;
                    rgb[idx + 1] = g_val;
                    rgb[idx + 2] = b_val;
                }
            }

            // Calculate square position (bouncing path or moving diagonal)
            let x_start = (f * 8) % (width - sq_size);
            let y_start = (f * 6) % (height - sq_size);

            for y in y_start..(y_start + sq_size) {
                let y_offset = y * width * 3;
                for x in x_start..(x_start + sq_size) {
                    let idx = (y_offset + x * 3) as usize;
                    // Draw a bright white square with a black border
                    if y == y_start
                        || y == y_start + sq_size - 1
                        || x == x_start
                        || x == x_start + sq_size - 1
                    {
                        rgb[idx] = 0;
                        rgb[idx + 1] = 0;
                        rgb[idx + 2] = 0;
                    } else {
                        rgb[idx] = 255;
                        rgb[idx + 1] = 255;
                        rgb[idx + 2] = 255;
                    }
                }
            }

            let encoded = encoder.encode(&rgb).unwrap();

            total_bytes += encoded.len();
            if f % 30 == 0 {
                println!("Frame {}: encoded size = {} bytes", f, encoded.len());
            }
            if !encoded.is_empty() {
                use std::io::Write;
                file.write_all(&encoded).unwrap();
            }
        }

        println!(
            "Generated moving test pattern H264 dump at: {:?}, total size: {} bytes",
            dump_path, total_bytes
        );
    }
}
