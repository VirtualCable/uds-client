// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebcamMode {
    MJPEG,
    H264,
    YUY2,
    Raw,
}

pub struct WebcamFrame {
    pub data: Vec<u8>,
    pub channel_ptr: usize,
}

pub trait WebcamIntegration: Send + Sync + std::fmt::Debug {
    fn is_h264_available(&self) -> bool;
    fn get_camera_dimensions(&self) -> (u32, u32);
    fn get_max_dimensions(&self) -> (u32, u32);
    fn get_fps(&self) -> u32;
    fn set_mode(&self, mode: WebcamMode);
    fn set_format(&self, format: u32, width: u32, height: u32, fps: u32);
    fn start_stream(&self, width: u32, height: u32, fps: u32) -> flume::Receiver<WebcamFrame>;
    fn stop_stream(&self);
    fn request_sample(&self, channel_ptr: usize);
    fn push_frame(&self, data: Vec<u8>);
    fn set_limits(&self, _quality: u32, _fps: u32, _max_width: u32, _max_height: u32) {}
}
