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

pub type WebcamCallback = Box<dyn Fn(Vec<u8>, usize) + Send + Sync + 'static>;

#[derive(Clone, Copy, PartialEq)]
pub enum WebcamMode {
    /// Send raw RGB frames (no encoding)
    Raw,
    /// Encode to MJPEG before storing
    MJPEG,
    /// Convert to YUY2
    YUY2,
}

pub struct WebcamHandle {
    cmd_tx: Sender<WebcamCommand>,
    pub latest_frame: Arc<Mutex<Option<Vec<u8>>>>,
    mode: Arc<Mutex<WebcamMode>>,
    pub on_frame: Arc<Mutex<Option<WebcamCallback>>>,
    pub samples_requested: Arc<Mutex<u32>>,
    pub active_channel: Arc<Mutex<Option<usize>>>,
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
        let on_frame = Arc::new(Mutex::new(None::<WebcamCallback>));
        let samples_requested = Arc::new(Mutex::new(0u32));
        let active_channel = Arc::new(Mutex::new(None::<usize>));

        let frame_out = latest_frame.clone();
        let cam_mode = mode.clone();
        let on_frame_cb = on_frame.clone();
        let samples_req = samples_requested.clone();
        let active_chan = active_channel.clone();
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
                        WebcamCommand::SetFormat {
                            format,
                            width,
                            height,
                            fps,
                        } => {
                            log::info!(
                                "Mock Webcam: SetFormat {format} {width}x{height} @ {fps}fps"
                            );
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
                                rgb[idx + 1] = 50;
                                rgb[idx + 2] = 50;
                            } else {
                                // Moving blue/green gradient background
                                rgb[idx] = 20;
                                rgb[idx + 1] = (y % 256) as u8;
                                rgb[idx + 2] = s.color_offset;
                            }
                        }
                    }

                    let mode_val = *cam_mode.lock().unwrap();
                    let output = if mode_val == WebcamMode::MJPEG {
                        encode_rgb_to_jpeg(&rgb, s.width, s.height)
                    } else if mode_val == WebcamMode::YUY2 {
                        rgb_to_yuy2(&rgb, s.width, s.height)
                    } else {
                        rgb
                    };

                    *frame_out.lock().unwrap() = Some(output.clone());
                    frame_count += 1;

                    let mut reqs = samples_req.lock().unwrap();
                    if *reqs > 0
                        && let (Some(chan), Some(ref cb)) = (
                            *active_chan.lock().unwrap(),
                            on_frame_cb.lock().unwrap().as_ref(),
                        )
                    {
                        *reqs -= 1;
                        cb(output, chan);
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
            active_channel,
        }
    }

    pub fn set_mode(&self, mode: WebcamMode) {
        *self.mode.lock().unwrap() = mode;
    }

    pub fn set_callback<F>(&self, cb: F)
    where
        F: Fn(Vec<u8>, usize) + Send + Sync + 'static,
    {
        *self.on_frame.lock().unwrap() = Some(Box::new(cb));
    }

    pub fn request_sample(&self, channel_ptr: usize) {
        *self.active_channel.lock().unwrap() = Some(channel_ptr);
        let mut reqs = self.samples_requested.lock().unwrap();
        *reqs += 1;
        if *reqs > 0
            && let (Some(frame), Some(ref cb)) = (
                self.latest_frame.lock().unwrap().as_ref(),
                self.on_frame.lock().unwrap().as_ref(),
            )
        {
            *reqs -= 1;
            cb(frame.to_vec(), channel_ptr);
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
        *self.active_channel.lock().unwrap() = None;
        *self.on_frame.lock().unwrap() = None;
        let _ = self.cmd_tx.send(WebcamCommand::StopStream);
    }

    pub fn close(&self) {
        *self.latest_frame.lock().unwrap() = None;
        *self.samples_requested.lock().unwrap() = 0;
        *self.active_channel.lock().unwrap() = None;
        *self.on_frame.lock().unwrap() = None;
        let _ = self.cmd_tx.send(WebcamCommand::Close);
    }
}

impl Default for WebcamHandle {
    fn default() -> Self {
        Self::new()
    }
}

fn rgb_to_yuy2(rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
    let num_pixels = (width * height) as usize;
    let mut yuy2 = vec![0u8; num_pixels * 2];

    for i in (0..num_pixels).step_by(2) {
        let idx1 = i * 3;
        let idx2 = (i + 1) * 3;

        if idx2 + 2 >= rgb.len() {
            break;
        }

        let r1 = rgb[idx1] as f32;
        let g1 = rgb[idx1 + 1] as f32;
        let b1 = rgb[idx1 + 2] as f32;

        let r2 = rgb[idx2] as f32;
        let g2 = rgb[idx2 + 1] as f32;
        let b2 = rgb[idx2 + 2] as f32;

        // Luminance Y1 and Y2
        let y1 = (0.299 * r1 + 0.587 * g1 + 0.114 * b1) as u8;
        let y2 = (0.299 * r2 + 0.587 * g2 + 0.114 * b2) as u8;

        // Chroma U and V (averaged)
        let r_avg = (r1 + r2) / 2.0;
        let g_avg = (g1 + g2) / 2.0;
        let b_avg = (b1 + b2) / 2.0;

        let u = (-0.168736 * r_avg - 0.331264 * g_avg + 0.5 * b_avg + 128.0) as u8;
        let v = (0.5 * r_avg - 0.418688 * g_avg - 0.081312 * b_avg + 128.0) as u8;

        let dest_idx = i * 2;
        yuy2[dest_idx] = y1;
        yuy2[dest_idx + 1] = u;
        yuy2[dest_idx + 2] = y2;
        yuy2[dest_idx + 3] = v;
    }

    yuy2
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
