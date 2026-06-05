use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use flume::{Sender, unbounded};
use nokhwa::{
    nokhwa_initialize,
    utils::{CameraFormat, FrameFormat, Resolution},
};
use shared::log;

mod mock;
mod encoders;
pub mod openh264;

pub use mock::{StreamState, generate_mock_frame};
pub use encoders::{VideoEncoder, RawEncoder, Yuy2Encoder, MjpegEncoder};
pub use openh264::h264_available;

pub static WEBCAM_QUALITY: AtomicU32 = AtomicU32::new(80);
pub static WEBCAM_FPS: AtomicU32 = AtomicU32::new(15);

pub enum WebcamCommand {
    StartStream {
        width: u32,
        height: u32,
        fps: u32,
    },
    /// Negotiate a new format. The capture loop will apply the closest match.
    SetFormat {
        format: u32,
        width: u32,
        height: u32,
        fps: u32,
    },
    StopStream,
    Close,
}

pub struct WebcamFrame {
    pub data: Vec<u8>,
    pub channel_ptr: usize,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum WebcamMode {
    /// Send raw RGB frames (no encoding)
    Raw,
    /// Encode to MJPEG before storing
    MJPEG,
    /// Convert to YUY2
    YUY2,
    /// Encode using OpenH264
    H264,
}

pub struct WebcamHandle {
    cmd_tx: Sender<WebcamCommand>,
    pub latest_frame: Arc<Mutex<Option<Vec<u8>>>>,
    mode: Arc<Mutex<WebcamMode>>,
    pub frame_tx: Arc<Mutex<Option<Sender<WebcamFrame>>>>,
    pub samples_requested: Arc<Mutex<u32>>,
    pub active_channel: Arc<Mutex<Option<usize>>>,
}

fn select_camera_index() -> nokhwa::utils::CameraIndex {
    if let Ok(val) = std::env::var("UDS_WEBCAM_DEVICE") {
        if let Ok(idx) = val.parse::<u32>() {
            return nokhwa::utils::CameraIndex::Index(idx);
        }
        // Query devices
        if let Ok(devices) = nokhwa::query(nokhwa::utils::ApiBackend::Auto) {
            let val_lower = val.to_lowercase();
            for camera in devices {
                if camera.human_name().to_lowercase().contains(&val_lower) {
                    return camera.index().clone();
                }
            }
        }
    }
    nokhwa::utils::CameraIndex::Index(0)
}

fn init_real_camera(width: u32, height: u32, fps: u32) -> Result<nokhwa::Camera, String> {
    let index = select_camera_index();
    
    let requested_none = nokhwa::utils::RequestedFormat::new::<nokhwa::pixel_format::RgbFormat>(
        nokhwa::utils::RequestedFormatType::None
    );
    let mut cam = nokhwa::Camera::new(index, requested_none)
        .map_err(|e| format!("Failed to create Camera: {e}"))?;
        
    if let Ok(formats) = cam.compatible_camera_formats() {
        let best_format = formats.iter().min_by_key(|f| {
            let res_diff = ((f.width() as i32 - width as i32).abs() + (f.height() as i32 - height as i32).abs()) as u32;
            let fps_diff = (f.frame_rate() as i32 - fps as i32).abs() as u32;
            (res_diff, fps_diff)
        });
        
        if let Some(&closest_format) = best_format {
            log::info!("Selected closest camera format: {:?}", closest_format);
            let requested_closest = nokhwa::utils::RequestedFormat::new::<nokhwa::pixel_format::RgbFormat>(
                nokhwa::utils::RequestedFormatType::Exact(closest_format)
            );
            let _ = cam.set_camera_requset(requested_closest);
        }
    }
    
    cam.open_stream()
        .map_err(|e| format!("Failed to open Camera stream: {e}"))?;
    
    Ok(cam)
}

impl WebcamHandle {
    pub fn new() -> Self {
        Self::with_mode(WebcamMode::Raw)
    }

    pub fn with_mode(mode: WebcamMode) -> Self {
        nokhwa_initialize(|_| {});

        let (cmd_tx, cmd_rx) = unbounded::<WebcamCommand>();
        let latest_frame = Arc::new(Mutex::new(None::<Vec<u8>>));
        let mode = Arc::new(Mutex::new(mode));
        let frame_tx = Arc::new(Mutex::new(None::<Sender<WebcamFrame>>));
        let samples_requested = Arc::new(Mutex::new(0u32));
        let active_channel = Arc::new(Mutex::new(None::<usize>));

        let frame_out = latest_frame.clone();
        let cam_mode = mode.clone();
        let frame_tx_cb = frame_tx.clone();
        let samples_req = samples_requested.clone();
        let active_chan = active_channel.clone();
        thread::spawn(move || {
            let mut state: Option<StreamState> = None;
            let mut frame_count: u64 = 0;
            let mut last_report = std::time::Instant::now();
            let mut encoder: Box<dyn VideoEncoder> = Box::new(RawEncoder);
            let mut current_mode: Option<WebcamMode> = None;
            let mut camera: Option<nokhwa::Camera> = None;
            let mut is_mock = false;

            loop {
                // Non-blocking query of commands
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        WebcamCommand::StartStream { width, height, fps } => {
                            log::info!("Webcam: StartStream {width}x{height} @ {fps}fps");
                            current_mode = None;
                            state = Some(StreamState {
                                width,
                                height,
                                fps,
                                format: 2, // Default to MJPEG format ID
                                color_offset: 0,
                            });

                            let force_mock = std::env::var("UDS_WEBCAM_MOCK").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false);
                            if force_mock {
                                log::info!("Mock Webcam forced by environment variable");
                                is_mock = true;
                                camera = None;
                            } else {
                                match init_real_camera(width, height, fps) {
                                    Ok(cam) => {
                                        camera = Some(cam);
                                        is_mock = false;
                                        log::info!("Real camera initialized successfully");
                                    }
                                    Err(e) => {
                                        log::warn!("Real camera initialization failed, falling back to mock: {}", e);
                                        camera = None;
                                        is_mock = true;
                                    }
                                }
                            }
                        }
                        WebcamCommand::SetFormat {
                            format: _,
                            width,
                            height,
                            fps,
                        } => {
                            log::info!(
                                "Webcam: SetFormat {width}x{height} @ {fps}fps"
                            );
                            current_mode = None;
                            if let Some(ref mut s) = state {
                                s.width = width;
                                s.height = height;
                                s.fps = fps;
                            }

                            if !is_mock {
                                if let Some(ref mut cam) = camera {
                                    let _ = cam.stop_stream();
                                }
                                match init_real_camera(width, height, fps) {
                                    Ok(cam) => {
                                        camera = Some(cam);
                                        is_mock = false;
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to re-initialize camera for new format, falling back to mock: {e}");
                                        camera = None;
                                        is_mock = true;
                                    }
                                }
                            }
                        }
                        WebcamCommand::StopStream => {
                            log::info!("Webcam: StopStream");
                            if let Some(mut cam) = camera.take() {
                                let _ = cam.stop_stream();
                            }
                            state = None;
                            *frame_out.lock().unwrap() = None;
                        }
                        WebcamCommand::Close => {
                            log::info!("Webcam: Close");
                            if let Some(mut cam) = camera.take() {
                                let _ = cam.stop_stream();
                            }
                            return;
                        }
                    }
                }

                if let Some(ref mut s) = state {
                    let rgb = if is_mock {
                        generate_mock_frame(s)
                    } else if let Some(ref mut cam) = camera {
                        match cam.frame() {
                            Ok(frame) => {
                                match frame.decode_image::<nokhwa::pixel_format::RgbFormat>() {
                                    Ok(img) => img.into_raw(),
                                    Err(e) => {
                                        log::error!("Failed to decode camera frame: {e}");
                                        generate_mock_frame(s)
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to capture camera frame: {e}");
                                generate_mock_frame(s)
                            }
                        }
                    } else {
                        generate_mock_frame(s)
                    };

                    let mode_val = *cam_mode.lock().unwrap();
                    if current_mode != Some(mode_val) {
                        encoder = match mode_val {
                            WebcamMode::MJPEG => Box::new(MjpegEncoder::new()),
                            WebcamMode::YUY2 => Box::new(Yuy2Encoder::new()),
                            WebcamMode::H264 => {
                                match encoders::H264Encoder::new() {
                                    Ok(enc) => Box::new(enc),
                                    Err(e) => {
                                        log::error!("Failed to create H264Encoder, falling back to MJPEG: {e}");
                                        Box::new(MjpegEncoder::new())
                                    }
                                }
                            }
                            WebcamMode::Raw => Box::new(RawEncoder),
                        };
                        let q = WEBCAM_QUALITY.load(Ordering::Relaxed);
                        let _ = encoder.init(s.width, s.height, s.fps, q);
                        current_mode = Some(mode_val);
                    }

                    let output = match encoder.encode(&rgb) {
                        Ok(out) => out,
                        Err(e) => {
                            log::error!("Webcam encoder error: {e}");
                            rgb.clone()
                        }
                    };

                    *frame_out.lock().unwrap() = Some(output.clone());
                    frame_count += 1;

                    let mut reqs = samples_req.lock().unwrap();
                    if *reqs > 0
                        && let (Some(chan), Some(tx)) = (
                            *active_chan.lock().unwrap(),
                            frame_tx_cb.lock().unwrap().as_ref(),
                        )
                    {
                        *reqs -= 1;
                        let _ = tx.send(WebcamFrame {
                            data: output,
                            channel_ptr: chan,
                        });
                    }

                    if last_report.elapsed().as_secs() >= 10 {
                        if frame_count > 0 {
                            log::debug!(
                                "Webcam: {} frames in 10s (~{:.1} fps)",
                                frame_count,
                                frame_count as f32 / 10.0,
                            );
                        }
                        frame_count = 0;
                        last_report = std::time::Instant::now();
                    }

                    let interval = Duration::from_secs_f64(1.0 / s.fps.max(1) as f64);
                    thread::sleep(interval);
                } else {
                    thread::sleep(Duration::from_millis(50));
                }
            }
        });

        WebcamHandle {
            cmd_tx,
            latest_frame,
            mode,
            frame_tx,
            samples_requested,
            active_channel,
        }
    }

    pub fn set_mode(&self, mode: WebcamMode) {
        *self.mode.lock().unwrap() = mode;
    }

    pub fn set_sender(&self, tx: Sender<WebcamFrame>) {
        *self.frame_tx.lock().unwrap() = Some(tx);
    }

    pub fn request_sample(&self, channel_ptr: usize) {
        *self.active_channel.lock().unwrap() = Some(channel_ptr);
        let mut reqs = self.samples_requested.lock().unwrap();
        *reqs += 1;
        if *reqs > 0
            && let (Some(frame), Some(tx)) = (
                self.latest_frame.lock().unwrap().as_ref(),
                self.frame_tx.lock().unwrap().as_ref(),
            )
        {
            *reqs -= 1;
            let _ = tx.send(WebcamFrame {
                data: frame.to_vec(),
                channel_ptr,
            });
        }
    }

    /// Query compatible formats from the default camera without keeping it open.
    /// Returns all (format, resolution, fps) tuples the hardware supports.
    pub fn compatible_formats() -> Vec<CameraFormat> {
        let force_mock = std::env::var("UDS_WEBCAM_MOCK").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false);
        if force_mock {
            return vec![
                CameraFormat::new(Resolution::new(640, 480), FrameFormat::MJPEG, 30),
                CameraFormat::new(Resolution::new(640, 480), FrameFormat::YUYV, 30),
                CameraFormat::new(Resolution::new(1280, 720), FrameFormat::MJPEG, 30),
                CameraFormat::new(Resolution::new(1920, 1080), FrameFormat::MJPEG, 30),
            ];
        }

        let index = select_camera_index();
        let requested = nokhwa::utils::RequestedFormat::new::<nokhwa::pixel_format::RgbFormat>(
            nokhwa::utils::RequestedFormatType::None
        );
        if let Ok(mut camera) = nokhwa::Camera::new(index, requested) {
            if let Ok(formats) = camera.compatible_camera_formats() {
                if !formats.is_empty() {
                    return formats;
                }
            }
        }

        // Return a mock set of formats if query fails
        vec![
            CameraFormat::new(Resolution::new(640, 480), FrameFormat::MJPEG, 30),
            CameraFormat::new(Resolution::new(640, 480), FrameFormat::YUYV, 30),
            CameraFormat::new(Resolution::new(1280, 720), FrameFormat::MJPEG, 30),
            CameraFormat::new(Resolution::new(1920, 1080), FrameFormat::MJPEG, 30),
        ]
    }

    /// Request a format change. The capture thread will apply the closest match.
    pub fn set_format(&self, format: u32, width: u32, height: u32, fps: u32) {
        let _ = self.cmd_tx.send(WebcamCommand::SetFormat {
            format,
            width,
            height,
            fps,
        });
    }

    pub fn start_stream(&self, width: u32, height: u32, fps: u32) {
        let _ = self
            .cmd_tx
            .send(WebcamCommand::StartStream { width, height, fps });
    }

    pub fn stop_stream(&self) {
        *self.latest_frame.lock().unwrap() = None;
        *self.samples_requested.lock().unwrap() = 0;
        *self.active_channel.lock().unwrap() = None;
        *self.frame_tx.lock().unwrap() = None;
        let _ = self.cmd_tx.send(WebcamCommand::StopStream);
    }

    pub fn close(&self) {
        *self.latest_frame.lock().unwrap() = None;
        *self.samples_requested.lock().unwrap() = 0;
        *self.active_channel.lock().unwrap() = None;
        *self.frame_tx.lock().unwrap() = None;
        let _ = self.cmd_tx.send(WebcamCommand::Close);
    }
}

impl Default for WebcamHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WebcamHandle {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(WebcamCommand::Close);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webcam_handle_creation() {
        let handle = WebcamHandle::new();
        assert!(handle.latest_frame.lock().unwrap().is_none());
        handle.close();
    }

    #[test]
    fn webcam_capture_frame() {
        let handle = WebcamHandle::new();
        handle.start_stream(320, 240, 5);
        std::thread::sleep(Duration::from_millis(500));
        let frame = handle.latest_frame.lock().unwrap().clone();
        if let Some(rgb) = frame {
            assert!(!rgb.is_empty());
        }
        handle.stop_stream();
        handle.close();
    }

    #[test]
    fn webcam_stop_clears_frame() {
        let handle = WebcamHandle::new();
        handle.start_stream(320, 240, 5);
        std::thread::sleep(Duration::from_millis(500));
        handle.stop_stream();

        let mut ok = false;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(10));
            if handle.latest_frame.lock().unwrap().is_none() {
                ok = true;
                break;
            }
        }
        assert!(ok);
        handle.close();
    }
}
