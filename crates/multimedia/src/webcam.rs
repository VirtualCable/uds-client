use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use flume::{Sender, unbounded};
use nokhwa::{
    Camera,
    nokhwa_initialize,
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
};
use shared::log;

pub enum WebcamCommand {
    StartStream { width: u32, height: u32, fps: u32 },
    StopStream,
    Close,
}

pub struct WebcamHandle {
    cmd_tx: Sender<WebcamCommand>,
    pub latest_frame: Arc<Mutex<Option<Vec<u8>>>>,
}

impl WebcamHandle {
    pub fn new() -> Self {
        nokhwa_initialize(|_| {});

        let (cmd_tx, cmd_rx) = unbounded::<WebcamCommand>();
        let latest_frame = Arc::new(Mutex::new(None::<Vec<u8>>));

        let frame_out = latest_frame.clone();
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
                                *frame_out.lock().unwrap() = Some(raw);
                            }
                            frame_count += 1;
                        }
                        Err(e) => {
                            log::trace!("Camera frame error: {e}");
                        }
                    }
                }

                // Periodic stats
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

                // Adaptive sleep: target the requested FPS
                let interval = Duration::from_secs_f64(1.0 / cap_fps.max(1) as f64);
                thread::sleep(interval);
            }
        });

        WebcamHandle {
            cmd_tx,
            latest_frame,
        }
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
