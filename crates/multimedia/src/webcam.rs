use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use flume::{Sender, unbounded};
use nokhwa::{
    Camera,
    nokhwa_initialize,
    pixel_format::RgbFormat,
    utils::{CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution},
};
use shared::log;
use turbojpeg::{Image, OutputBuf, PixelFormat};

pub enum WebcamCommand {
    StartStream { width: u32, height: u32, fps: u32 },
    /// Negotiate a new format. The capture loop will apply the closest match.
    SetFormat { format: u32, width: u32, height: u32, fps: u32 },
    StopStream,
    Close,
}

#[derive(Clone, Copy, PartialEq)]
pub enum WebcamMode {
    /// Send raw RGB frames (no encoding)
    Raw,
    /// Encode to MJPEG before storing
    MJPEG,
}

pub struct WebcamHandle {
    cmd_tx: Sender<WebcamCommand>,
    pub latest_frame: Arc<Mutex<Option<Vec<u8>>>>,
    mode: Arc<Mutex<WebcamMode>>,
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

        let frame_out = latest_frame.clone();
        let cam_mode = mode.clone();
        thread::spawn(move || {
            let mut camera: Option<Camera> = None;
            let mut cap_fps: u32 = 15;
            let mut frame_count: u64 = 0;
            let mut last_report = std::time::Instant::now();

            loop {
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        WebcamCommand::StartStream { width: _, height: _, fps } => {
                            cap_fps = fps;
                            if camera.is_none() {
                                let requested = RequestedFormat::new::<RgbFormat>(
                                    RequestedFormatType::AbsoluteHighestFrameRate,
                                );
                                match Camera::new(CameraIndex::Index(0), requested) {
                                    Ok(cam) => {
                                        log::info!(
                                            "Webcam opened: {}x{} @ {}fps",
                                            cam.camera_format().width(),
                                            cam.camera_format().height(),
                                            cam.camera_format().frame_rate(),
                                        );
                                        camera = Some(cam);
                                    }
                                    Err(e) => {
                                        log::error!("Failed to open webcam: {e}");
                                    }
                                }
                            }
                        }
                        WebcamCommand::SetFormat { format, width, height, fps } => {
                            cap_fps = fps;
                            if let Some(ref mut cam) = camera {
                                apply_format(cam, format, width, height, fps);
                            }
                        }
                        WebcamCommand::StopStream => {
                            camera = None;
                            *frame_out.lock().unwrap() = None;
                        }
                        WebcamCommand::Close => return,
                    }
                }

                if let Some(ref mut cam) = camera {
                    match cam.frame() {
                        Ok(frame) => {
                            let raw = frame.buffer().to_vec();
                            if !raw.is_empty() {
                                let output = if *cam_mode.lock().unwrap() == WebcamMode::MJPEG {
                                    let fmt = cam.camera_format();
                                    encode_rgb_to_jpeg(&raw, fmt.width(), fmt.height())
                                } else {
                                    raw
                                };
                                *frame_out.lock().unwrap() = Some(output);
                            }
                            frame_count += 1;
                        }
                        Err(e) => {
                            log::trace!("Camera frame error: {e}");
                        }
                    }
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

                let interval = Duration::from_secs_f64(1.0 / cap_fps.max(1) as f64);
                thread::sleep(interval);
            }
        });

        WebcamHandle {
            cmd_tx,
            latest_frame,
            mode,
        }
    }

    pub fn set_mode(&self, mode: WebcamMode) {
        *self.mode.lock().unwrap() = mode;
    }

    /// Query compatible formats from the default camera without keeping it open.
    /// Returns all (format, resolution, fps) tuples the hardware supports.
    pub fn compatible_formats() -> Vec<CameraFormat> {
        nokhwa_initialize(|_| {});
        let requested = RequestedFormat::new::<RgbFormat>(
            RequestedFormatType::None,
        );
        match Camera::new(CameraIndex::Index(0), requested) {
            Ok(mut cam) => cam.compatible_camera_formats().unwrap_or_default(),
            Err(_) => Vec::new(),
        }
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
        let _ = self.cmd_tx.send(WebcamCommand::StopStream);
    }

    pub fn close(&self) {
        let _ = self.cmd_tx.send(WebcamCommand::Close);
    }
}

impl Default for WebcamHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Map a RDPECAM CAM_MEDIA_FORMAT code to nokhwa FrameFormat preference order.
fn media_format_to_frame_formats(format: u32) -> &'static [FrameFormat] {
    match format {
        0x02 => &[FrameFormat::MJPEG, FrameFormat::YUYV, FrameFormat::RAWRGB], // MJPG
        0x03 => &[FrameFormat::YUYV, FrameFormat::RAWRGB, FrameFormat::MJPEG], // YUY2
        0x04 => &[FrameFormat::NV12, FrameFormat::YUYV, FrameFormat::RAWRGB], // NV12
        0x06 => &[FrameFormat::RAWRGB, FrameFormat::YUYV, FrameFormat::MJPEG], // RGB24
        _ => &[FrameFormat::MJPEG, FrameFormat::YUYV, FrameFormat::RAWRGB, FrameFormat::NV12],
    }
}

/// Apply the closest matching format to the camera.
fn apply_format(cam: &mut Camera, format: u32, width: u32, height: u32, fps: u32) {
    let preferred = media_format_to_frame_formats(format);
    // Build target CameraFormat
    let target = CameraFormat::new(
        Resolution::new(width, height),
        preferred[0],
        fps,
    );

    let requested = RequestedFormat::with_formats(
        RequestedFormatType::Closest(target),
        preferred,
    );

    match cam.set_camera_requset(requested) {
        Ok(actual) => {
            log::info!(
                "Webcam: format set to {}x{} @ {}fps {:?}",
                actual.width(),
                actual.height(),
                actual.frame_rate(),
                actual.format(),
            );
        }
        Err(e) => {
            log::error!("Webcam: set_camera_requset failed: {e}");
        }
    }
}

/// Encode an RGB buffer to JPEG using turbojpeg.
fn encode_rgb_to_jpeg(rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
    match turbojpeg::Compressor::new() {
        Ok(mut compressor) => {
            let image = Image {
                pixels: rgb,
                width: width as usize,
                height: height as usize,
                pitch: (width * 3) as usize,
                format: PixelFormat::RGB,
            };
            let mut output = OutputBuf::new_owned();
            if compressor.compress(image, &mut output).is_ok() {
                let jpeg = output.to_vec();
                if !jpeg.is_empty() {
                    return jpeg;
                }
            }
        }
        Err(e) => {
            log::error!("Webcam JPEG encoder init failed: {e}");
        }
    }
    // Fallback: return raw RGB (server may handle raw if encoding fails)
    rgb.to_vec()
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
    #[ignore] // Requires a real webcam, skip in CI
    fn webcam_capture_frame() {
        let handle = WebcamHandle::new();
        handle.start_stream(320, 240, 5);
        std::thread::sleep(Duration::from_millis(500));
        let frame = handle.latest_frame.lock().unwrap().clone();
        if let Some(rgb) = frame {
            assert!(!rgb.is_empty());
            assert_eq!(rgb.len(), (320 * 240 * 3) as usize);
        }
        handle.stop_stream();
        handle.close();
    }

    #[test]
    #[ignore] // Requires a real webcam
    fn webcam_stop_clears_frame() {
        let handle = WebcamHandle::new();
        handle.start_stream(320, 240, 5);
        std::thread::sleep(Duration::from_millis(500));
        handle.stop_stream();
        std::thread::sleep(Duration::from_millis(100));
        assert!(handle.latest_frame.lock().unwrap().is_none());
        handle.close();
    }
}
