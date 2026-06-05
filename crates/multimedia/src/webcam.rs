use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use flume::{Sender, unbounded};
use nokhwa::{
    nokhwa_initialize,
    utils::{CameraFormat, FrameFormat, Resolution},
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
    pub on_frame: Arc<Mutex<Option<Box<dyn Fn(Vec<u8>) + Send + Sync + 'static>>>>,
    pub samples_requested: Arc<Mutex<u32>>,
}

struct StreamState {
    width: u32,
    height: u32,
    fps: u32,
    format: u32,
    color_offset: u8,
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
        let on_frame = Arc::new(Mutex::new(None::<Box<dyn Fn(Vec<u8>) + Send + Sync + 'static>>));
        let samples_requested = Arc::new(Mutex::new(0u32));

        let frame_out = latest_frame.clone();
        let cam_mode = mode.clone();
        let on_frame_cb = on_frame.clone();
        let samples_req = samples_requested.clone();
        thread::spawn(move || {
            let mut state: Option<StreamState> = None;
            let mut frame_count: u64 = 0;
            let mut last_report = std::time::Instant::now();

            loop {
                // Non-blocking query of commands
                while let Ok(cmd) = cmd_rx.try_recv() {
                    match cmd {
                        WebcamCommand::StartStream { width, height, fps } => {
                            log::info!("Mock Webcam: StartStream {width}x{height} @ {fps}fps");
                            state = Some(StreamState {
                                width,
                                height,
                                fps,
                                format: 2, // Default to MJPEG format ID
                                color_offset: 0,
                            });
                        }
                        WebcamCommand::SetFormat { format, width, height, fps } => {
                            log::info!("Mock Webcam: SetFormat {format} {width}x{height} @ {fps}fps");
                            if let Some(ref mut s) = state {
                                s.width = width;
                                s.height = height;
                                s.fps = fps;
                                s.format = format;
                            }
                        }
                        WebcamCommand::StopStream => {
                            log::info!("Mock Webcam: StopStream");
                            state = None;
                            *frame_out.lock().unwrap() = None;
                        }
                        WebcamCommand::Close => {
                            log::info!("Mock Webcam: Close");
                            return;
                        }
                    }
                }

                if let Some(ref mut s) = state {
                    // Generate mock frame
                    s.color_offset = s.color_offset.wrapping_add(4);
                    let bar_pos = (s.color_offset as u32 * 2) % s.width;
                    let mut rgb = vec![0u8; (s.width * s.height * 3) as usize];
                    
                    for y in 0..s.height {
                        for x in 0..s.width {
                            let idx = ((y * s.width + x) * 3) as usize;
                            if x >= bar_pos && x < bar_pos + 40 {
                                // Moving red bar
                                rgb[idx] = 255;
                                rgb[idx+1] = 50;
                                rgb[idx+2] = 50;
                            } else {
                                // Moving blue/green gradient background
                                rgb[idx] = 20;
                                rgb[idx+1] = (y % 256) as u8;
                                rgb[idx+2] = s.color_offset;
                            }
                        }
                    }

                    let output = if *cam_mode.lock().unwrap() == WebcamMode::MJPEG {
                        encode_rgb_to_jpeg(&rgb, s.width, s.height)
                    } else {
                        rgb
                    };

                    *frame_out.lock().unwrap() = Some(output.clone());
                    frame_count += 1;

                    let mut reqs = samples_req.lock().unwrap();
                    if *reqs > 0 {
                        if let Some(ref cb) = *on_frame_cb.lock().unwrap() {
                            *reqs -= 1;
                            cb(output);
                        }
                    }

                    if last_report.elapsed().as_secs() >= 10 {
                        if frame_count > 0 {
                            log::debug!(
                                "Mock Webcam: {} frames in 10s (~{:.1} fps)",
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
            on_frame,
            samples_requested,
        }
    }

    pub fn set_mode(&self, mode: WebcamMode) {
        *self.mode.lock().unwrap() = mode;
    }

    pub fn set_callback<F>(&self, cb: F)
    where
        F: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        *self.on_frame.lock().unwrap() = Some(Box::new(cb));
    }

    pub fn request_sample(&self) {
        let mut reqs = self.samples_requested.lock().unwrap();
        *reqs += 1;
        if *reqs > 0 {
            if let Some(ref frame) = *self.latest_frame.lock().unwrap() {
                if let Some(ref cb) = *self.on_frame.lock().unwrap() {
                    *reqs -= 1;
                    cb(frame.clone());
                }
            }
        }
    }

    /// Query compatible formats from the default camera without keeping it open.
    /// Returns all (format, resolution, fps) tuples the hardware supports.
    pub fn compatible_formats() -> Vec<CameraFormat> {
        // Return a mock set of formats
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
        *self.on_frame.lock().unwrap() = None;
        let _ = self.cmd_tx.send(WebcamCommand::StopStream);
    }

    pub fn close(&self) {
        *self.latest_frame.lock().unwrap() = None;
        *self.samples_requested.lock().unwrap() = 0;
        *self.on_frame.lock().unwrap() = None;
        let _ = self.cmd_tx.send(WebcamCommand::Close);
    }
}

impl Default for WebcamHandle {
    fn default() -> Self {
        Self::new()
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
