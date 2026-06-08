use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use flume::{Sender, unbounded};
use nokhwa::{
    nokhwa_initialize,
    utils::{CameraFormat, FrameFormat, Resolution},
};
use shared::log;

mod encoders;
mod mock;
pub mod openh264;

pub use encoders::{MjpegEncoder, RawEncoder, VideoEncoder, Yuy2Encoder};
pub use mock::{StreamState, generate_mock_frame};
pub use openh264::h264_available;

// We use static values as we can only have ONE connection.
pub static WEBCAM_QUALITY: AtomicU32 = AtomicU32::new(80);
pub static WEBCAM_FPS: AtomicU32 = AtomicU32::new(15);
pub static WEBCAM_MAX_WIDTH: AtomicU32 = AtomicU32::new(0);
pub static WEBCAM_MAX_HEIGHT: AtomicU32 = AtomicU32::new(0);

static CAMERA_DIMENSIONS: std::sync::OnceLock<Option<(u32, u32)>> = std::sync::OnceLock::new();

pub fn get_camera_dimensions() -> Option<(u32, u32)> {
    *CAMERA_DIMENSIONS.get_or_init(|| {
        let force_mock = std::env::var("UDSLAUNCHER_CAM_MOCK")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
        if force_mock {
            return Some((640, 480));
        }

        nokhwa_initialize(|_| {});

        // Check if there are any cameras on the system
        let devices = nokhwa::query(nokhwa::utils::ApiBackend::Auto)
            .ok()
            .unwrap_or_default();
        if devices.is_empty() {
            log::warn!("No cameras detected on the system via query.");
            return None;
        }

        let index = select_camera_index();
        let requested_none = nokhwa::utils::RequestedFormat::new::<nokhwa::pixel_format::RgbFormat>(
            nokhwa::utils::RequestedFormatType::None,
        );
        if let Ok(mut cam) = nokhwa::Camera::new(index, requested_none)
            && let Ok(formats) = cam.compatible_camera_formats()
        {
            let best_format = formats.iter().max_by_key(|f| f.width() * f.height());
            if let Some(format) = best_format {
                log::info!(
                    "Proactively detected camera dimensions: {}x{}",
                    format.width(),
                    format.height()
                );
                return Some((format.width(), format.height()));
            }
        }

        // If we have devices but failed to open or query formats (e.g. camera in use),
        // fallback to default dimensions to keep redirection enabled.
        log::warn!("Camera detected but failed to query dimensions. Falling back to 640x480.");
        Some((640, 480))
    })
}

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
    if let Ok(val) = std::env::var("UDSLAUNCHER_CAM_DEVICE") {
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
        nokhwa::utils::RequestedFormatType::None,
    );
    let mut cam = nokhwa::Camera::new(index, requested_none)
        .map_err(|e| format!("Failed to create Camera: {e}"))?;

    if let Ok(formats) = cam.compatible_camera_formats() {
        log::debug!("Webcam: All compatible camera formats: {:?}", formats);
        let best_format = formats.iter().min_by_key(|f| {
            let res_diff = (f.width() as i32 - width as i32).unsigned_abs()
                + (f.height() as i32 - height as i32).unsigned_abs();
            let fps_diff = (f.frame_rate() as i32 - fps as i32).unsigned_abs();
            (res_diff, fps_diff)
        });

        if let Some(&closest_format) = best_format {
            log::info!("Selected closest camera format: {:?}", closest_format);
            let requested_closest =
                nokhwa::utils::RequestedFormat::new::<nokhwa::pixel_format::RgbFormat>(
                    nokhwa::utils::RequestedFormatType::Exact(closest_format),
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
            let mut bytes_count: u64 = 0;
            let mut last_report = std::time::Instant::now();
            let mut stream_start_time = std::time::Instant::now();
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
                            frame_count = 0;
                            bytes_count = 0;
                            last_report = std::time::Instant::now();
                            stream_start_time = std::time::Instant::now();
                            state = Some(StreamState {
                                width,
                                height,
                                fps,
                                format: 2, // Default to MJPEG format ID
                                color_offset: 0,
                            });

                            let force_mock = std::env::var("UDSLAUNCHER_CAM_MOCK")
                                .map(|v| v == "1" || v.to_lowercase() == "true")
                                .unwrap_or(false);
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
                                        log::warn!(
                                            "Real camera initialization failed, falling back to mock: {}",
                                            e
                                        );
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
                            log::info!("Webcam: SetFormat {width}x{height} @ {fps}fps");
                            current_mode = None;
                            let mut needs_restart = true;
                            if let Some(ref mut s) = state {
                                if s.width == width && s.height == height && s.fps == fps && camera.is_some() {
                                    needs_restart = false;
                                    log::info!("Webcam: Format matches current stream, skipping camera restart");
                                } else {
                                    s.width = width;
                                    s.height = height;
                                    s.fps = fps;
                                }
                            }

                            if !is_mock && needs_restart {
                                if let Some(ref mut cam) = camera {
                                    let _ = cam.stop_stream();
                                }
                                match init_real_camera(width, height, fps) {
                                    Ok(cam) => {
                                        camera = Some(cam);
                                        is_mock = false;
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "Failed to re-initialize camera for new format, falling back to mock: {e}"
                                        );
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
                    log::trace!("Webcam capture loop iteration: frame_count = {}", frame_count);
                    let (rgb, src_w, src_h) = if is_mock {
                        (generate_mock_frame(s), s.width, s.height)
                    } else if let Some(ref mut cam) = camera {
                        log::trace!("Calling cam.frame()...");
                        match cam.frame() {
                            Ok(frame) => {
                                log::trace!("cam.frame() returned Ok");
                                match frame.decode_image::<nokhwa::pixel_format::RgbFormat>() {
                                    Ok(img) => {
                                        let w = img.width();
                                        let h = img.height();
                                        (img.into_raw(), w, h)
                                    }
                                    Err(e) => {
                                        log::error!("Failed to decode camera frame: {e}");
                                        (generate_mock_frame(s), s.width, s.height)
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to capture camera frame: {e}");
                                (generate_mock_frame(s), s.width, s.height)
                            }
                        }
                    } else {
                        (generate_mock_frame(s), s.width, s.height)
                    };

                    let (dst_w, dst_h) = calculate_scaled_dimensions(s.width, s.height);
                    let rgb_scaled = resize_rgb(&rgb, src_w, src_h, dst_w, dst_h);

                    let mode_val = *cam_mode.lock().unwrap();
                    if current_mode != Some(mode_val) {
                        encoder = match mode_val {
                            WebcamMode::MJPEG => Box::new(MjpegEncoder::new()),
                            WebcamMode::YUY2 => Box::new(Yuy2Encoder::new()),
                            WebcamMode::H264 => match encoders::H264Encoder::new() {
                                Ok(enc) => Box::new(enc),
                                Err(e) => {
                                    log::error!(
                                        "Failed to create H264Encoder, falling back to MJPEG: {e}"
                                    );
                                    Box::new(MjpegEncoder::new())
                                }
                            },
                            WebcamMode::Raw => Box::new(RawEncoder),
                        };
                        let q = WEBCAM_QUALITY.load(Ordering::Relaxed);
                        let _ = encoder.init(dst_w, dst_h, s.fps, q);
                        current_mode = Some(mode_val);
                        log::info!(
                            "Webcam encoder initialized: Mode={:?}, Resolution={}x{}, FPS={}",
                            mode_val,
                            dst_w,
                            dst_h,
                            s.fps
                        );
                    }

                    let output = match encoder.encode(&rgb_scaled) {
                        Ok(out) => out,
                        Err(e) => {
                            log::error!("Webcam encoder error: {e}");
                            rgb_scaled.clone()
                        }
                    };

                    *frame_out.lock().unwrap() = Some(output.clone());
                    frame_count += 1;
                    bytes_count += output.len() as u64;

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

                    let elapsed_total = stream_start_time.elapsed().as_secs();
                    let report_interval = if elapsed_total <= 60 {
                        5
                    } else if elapsed_total <= 120 {
                        15
                    } else if elapsed_total <= 240 {
                        30
                    } else {
                        60
                    };

                    let duration = last_report.elapsed().as_secs_f64();
                    if duration >= report_interval as f64 {
                        if frame_count > 0 {
                            let fps = frame_count as f64 / duration;
                            let bytes_per_sec = bytes_count as f64 / duration;
                            let bytes_per_frame = bytes_count as f64 / frame_count as f64;
                            let mode_name = match current_mode {
                                Some(WebcamMode::MJPEG) => "MJPEG",
                                Some(WebcamMode::H264) => "H264",
                                Some(WebcamMode::YUY2) => "YUY2",
                                Some(WebcamMode::Raw) => "Raw",
                                None => "None",
                            };
                            log::debug!(
                                "Webcam [{}]: {} frames in {:.1}s (~{:.1} fps), {:.0} bytes/s, {:.0} bytes/frame",
                                mode_name,
                                frame_count,
                                duration,
                                fps,
                                bytes_per_sec,
                                bytes_per_frame,
                            );
                        }
                        frame_count = 0;
                        bytes_count = 0;
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
        
        let current_mode = *self.mode.lock().unwrap();
        if current_mode != WebcamMode::H264
            && *reqs > 0
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
        let force_mock = std::env::var("UDSLAUNCHER_CAM_MOCK")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
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
            nokhwa::utils::RequestedFormatType::None,
        );
        if let Ok(mut camera) = nokhwa::Camera::new(index, requested)
            && let Ok(formats) = camera.compatible_camera_formats()
            && !formats.is_empty()
        {
            return formats;
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

fn calculate_scaled_dimensions(w: u32, h: u32) -> (u32, u32) {
    let max_w = WEBCAM_MAX_WIDTH.load(Ordering::Relaxed);
    let max_h = WEBCAM_MAX_HEIGHT.load(Ordering::Relaxed);

    let mut target_w = w;
    let mut target_h = h;

    if max_w > 0 && target_w > max_w {
        target_h = (target_h * max_w) / target_w;
        target_w = max_w;
    }

    if max_h > 0 && target_h > max_h {
        target_w = (target_w * max_h) / target_h;
        target_h = max_h;
    }

    // Ensure even dimensions (required by H264 and many encoders)
    target_w = (target_w / 2) * 2;
    target_h = (target_h / 2) * 2;

    (target_w.max(2), target_h.max(2))
}

fn resize_rgb(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    if src_w == dst_w && src_h == dst_h {
        return src.to_vec();
    }
    let mut dst = vec![0u8; (dst_w * dst_h * 3) as usize];
    for y in 0..dst_h {
        let py = ((y * src_h) / dst_h).min(src_h - 1) as usize;
        let py_offset = py * src_w as usize * 3;
        let dst_y_offset = y as usize * dst_w as usize * 3;
        for x in 0..dst_w {
            let px = ((x * src_w) / dst_w).min(src_w - 1) as usize;
            let src_idx = py_offset + px * 3;
            let dst_idx = dst_y_offset + x as usize * 3;

            dst[dst_idx] = src[src_idx];
            dst[dst_idx + 1] = src[src_idx + 1];
            dst[dst_idx + 2] = src[src_idx + 2];
        }
    }
    dst
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

    #[test]
    fn test_calculate_scaled_dimensions() {
        // Default with no limits (both 0)
        WEBCAM_MAX_WIDTH.store(0, Ordering::Relaxed);
        WEBCAM_MAX_HEIGHT.store(0, Ordering::Relaxed);
        let (w, h) = calculate_scaled_dimensions(640, 480);
        assert_eq!(w, 640);
        assert_eq!(h, 480);

        // Limit width
        WEBCAM_MAX_WIDTH.store(320, Ordering::Relaxed);
        let (w, h) = calculate_scaled_dimensions(640, 480);
        assert_eq!(w, 320);
        assert_eq!(h, 240); // 480 * 320 / 640

        // Limit height
        WEBCAM_MAX_WIDTH.store(0, Ordering::Relaxed);
        WEBCAM_MAX_HEIGHT.store(200, Ordering::Relaxed);
        let (w, h) = calculate_scaled_dimensions(640, 480);
        assert_eq!(w, 266); // 640 * 200 / 480 = 266.66 -> even rounding -> 266
        assert_eq!(h, 200);

        // Restore defaults
        WEBCAM_MAX_WIDTH.store(0, Ordering::Relaxed);
        WEBCAM_MAX_HEIGHT.store(0, Ordering::Relaxed);
    }

    #[test]
    fn test_resize_rgb() {
        let src = vec![1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]; // 2x2 RGB image
        let dst = resize_rgb(&src, 2, 2, 1, 1); // scale down to 1x1
        assert_eq!(dst.len(), 3);
        assert_eq!(dst, vec![1u8, 2, 3]); // nearest neighbor maps top-left pixel
    }

    #[test]
    fn test_get_camera_dimensions() {
        unsafe {
            std::env::set_var("UDSLAUNCHER_CAM_MOCK", "true");
        }
        let dims = get_camera_dimensions();
        assert_eq!(dims, Some((640, 480)));
        unsafe {
            std::env::remove_var("UDSLAUNCHER_CAM_MOCK");
        }
    }
}
