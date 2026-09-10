// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use flume::{Receiver, Sender};
use shared::log;

use crate::webcam::encoders::{self, MjpegEncoder, RawEncoder, VideoEncoder, Yuy2Encoder};
use crate::webcam::{
    StreamState, WEBCAM_QUALITY, WebcamCommand, WebcamFrame, WebcamMode,
    calculate_scaled_dimensions, effective_fps, generate_mock_frame, init_real_camera, resize_rgb,
};

fn next_deadline(previous: Instant, interval: Duration, now: Instant) -> Instant {
    (previous + interval).max(now)
}

/// How long to wait before trying to reopen a camera that stopped delivering frames.
/// Windows only releases the hardware once the previous instance is dropped, so the
/// camera is dropped on failure and reacquired on this cadence for as long as the
/// stream lasts: whatever was holding it (another app, a USB reset, a driver
/// reconfiguration) usually lets go on its own.
const CAMERA_REOPEN_INTERVAL: Duration = Duration::from_secs(2);

fn reopen_due(retry_at: Option<Instant>, now: Instant) -> bool {
    retry_at.is_some_and(|at| now >= at)
}

/// Holds all channels and state shared with the [`WebcamHandle`](crate::webcam::WebcamHandle).
pub(crate) struct CaptureLoop {
    cmd_rx: Receiver<WebcamCommand>,
    frame_out: Arc<Mutex<Option<Vec<u8>>>>,
    cam_mode: Arc<Mutex<WebcamMode>>,
    frame_tx_cb: Arc<Mutex<Option<Sender<WebcamFrame>>>>,
    samples_req: Arc<Mutex<u32>>,
    active_chan: Arc<Mutex<Option<usize>>>,
}

impl CaptureLoop {
    pub(crate) fn new(
        cmd_rx: Receiver<WebcamCommand>,
        frame_out: Arc<Mutex<Option<Vec<u8>>>>,
        cam_mode: Arc<Mutex<WebcamMode>>,
        frame_tx_cb: Arc<Mutex<Option<Sender<WebcamFrame>>>>,
        samples_req: Arc<Mutex<u32>>,
        active_chan: Arc<Mutex<Option<usize>>>,
    ) -> Self {
        Self {
            cmd_rx,
            frame_out,
            cam_mode,
            frame_tx_cb,
            samples_req,
            active_chan,
        }
    }

    /// Spawns a thread that runs the capture loop until a `Close` command is
    /// received. Consumes `self` so the channels are dropped when the thread exits.
    pub(crate) fn run(self) {
        thread::spawn(move || {
            let mut state: Option<StreamState> = None;
            let mut frame_count: u64 = 0;
            let mut bytes_count: u64 = 0;
            let mut last_report = std::time::Instant::now();
            let mut stream_start_time = std::time::Instant::now();
            let mut next_frame_at = Instant::now();
            let mut encoder: Box<dyn VideoEncoder> = Box::new(RawEncoder);
            let mut current_mode: Option<WebcamMode> = None;
            let mut camera: Option<(nokhwa::Camera, u32)> = None;
            let mut camera_retry_at: Option<Instant> = None;
            let mut is_mock = false;

            loop {
                // Non-blocking query of commands
                while let Ok(cmd) = self.cmd_rx.try_recv() {
                    match cmd {
                        WebcamCommand::StartStream { width, height, fps } => {
                            log::debug!("Webcam: StartStream {width}x{height} @ {fps}fps");
                            // Media Foundation hands out the camera exclusively: the previous
                            // instance has to be dropped before another one is opened, or the
                            // open is denied.
                            if let Some((mut cam, _)) = camera.take() {
                                let _ = cam.stop_stream();
                            }
                            current_mode = None;
                            frame_count = 0;
                            bytes_count = 0;
                            last_report = std::time::Instant::now();
                            stream_start_time = std::time::Instant::now();
                            next_frame_at = Instant::now();
                            camera_retry_at = None;
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
                            } else {
                                match init_real_camera(width, height, fps) {
                                    Ok((cam, camera_fps)) => {
                                        let real_fps = effective_fps(fps, camera_fps);
                                        if real_fps != fps {
                                            log::info!(
                                                "Webcam: {fps}fps requested, camera delivers {real_fps}fps; streaming at {real_fps}fps"
                                            );
                                            if let Some(ref mut s) = state {
                                                s.fps = real_fps;
                                            }
                                        }
                                        camera = Some((cam, camera_fps));
                                        is_mock = false;
                                        log::debug!("Real camera initialized successfully");
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
                            let fps = effective_fps(
                                fps,
                                camera.as_ref().map_or(0, |(_, camera_fps)| *camera_fps),
                            );
                            log::debug!("Webcam: SetFormat {width}x{height} @ {fps}fps");
                            current_mode = None;
                            let mut needs_restart = true;
                            if let Some(ref mut s) = state {
                                if s.width == width
                                    && s.height == height
                                    && s.fps == fps
                                    && camera.is_some()
                                {
                                    needs_restart = false;
                                    log::debug!(
                                        "Webcam: Format matches current stream, skipping camera restart"
                                    );
                                } else {
                                    s.width = width;
                                    s.height = height;
                                    s.fps = fps;
                                }
                            }

                            if !is_mock && needs_restart {
                                // Stopping the stream is not enough: the camera stays alive until
                                // it is dropped, and Media Foundation denies a second open while
                                // the old instance holds the hardware.
                                if let Some((mut cam, _)) = camera.take() {
                                    let _ = cam.stop_stream();
                                }
                                match init_real_camera(width, height, fps) {
                                    Ok((cam, camera_fps)) => {
                                        if let Some(ref mut s) = state {
                                            s.fps = effective_fps(fps, camera_fps);
                                        }
                                        camera = Some((cam, camera_fps));
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
                            log::debug!("Webcam: StopStream");
                            if let Some((mut cam, _)) = camera.take() {
                                let _ = cam.stop_stream();
                            }
                            camera_retry_at = None;
                            state = None;
                            *self.frame_out.lock().unwrap() = None;
                        }
                        WebcamCommand::Close => {
                            log::debug!("Webcam: Close");
                            if let Some((mut cam, _)) = camera.take() {
                                let _ = cam.stop_stream();
                            }
                            return;
                        }
                    }
                }

                if let Some(ref mut s) = state {
                    log::trace!(
                        "Webcam capture loop iteration: frame_count = {}",
                        frame_count
                    );
                    if !is_mock && camera.is_none() && reopen_due(camera_retry_at, Instant::now()) {
                        match init_real_camera(s.width, s.height, s.fps) {
                            Ok((cam, camera_fps)) => {
                                log::info!("Webcam re-acquired successfully");
                                s.fps = effective_fps(s.fps, camera_fps);
                                camera = Some((cam, camera_fps));
                                camera_retry_at = None;
                                current_mode = None;
                            }
                            Err(e) => {
                                log::debug!("Webcam: camera still unavailable: {e}");
                                camera_retry_at = Some(Instant::now() + CAMERA_REOPEN_INTERVAL);
                            }
                        }
                    }

                    let grabbed = if is_mock {
                        None
                    } else {
                        camera.as_mut().map(|(cam, _)| {
                            log::trace!("Calling cam.frame()...");
                            cam.frame()
                        })
                    };

                    let (rgb, src_w, src_h) = match grabbed {
                        Some(Ok(frame)) => {
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
                        Some(Err(e)) => {
                            log::error!(
                                "Failed to capture camera frame: {e}. Serving mock frames and retrying every {}s",
                                CAMERA_REOPEN_INTERVAL.as_secs()
                            );
                            camera = None;
                            camera_retry_at = Some(Instant::now() + CAMERA_REOPEN_INTERVAL);
                            (generate_mock_frame(s), s.width, s.height)
                        }
                        None => (generate_mock_frame(s), s.width, s.height),
                    };

                    let mode_val = *self.cam_mode.lock().unwrap();
                    // H264 predicts every frame from the previous one, so encoding a frame
                    // that is then discarded leaves the server decoding against a reference
                    // it never received: the picture smears until the next IDR.
                    let sample_pending = *self.samples_req.lock().unwrap() > 0;
                    if mode_val != WebcamMode::H264 || sample_pending {
                        let (dst_w, dst_h) = calculate_scaled_dimensions(s.width, s.height);
                        let rgb_scaled = resize_rgb(&rgb, src_w, src_h, dst_w, dst_h);

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
                            let q = WEBCAM_QUALITY.load(std::sync::atomic::Ordering::Relaxed);
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

                        *self.frame_out.lock().unwrap() = Some(output.clone());
                        frame_count += 1;
                        bytes_count += output.len() as u64;

                        let mut reqs = self.samples_req.lock().unwrap();
                        if *reqs > 0
                            && let (Some(chan), Some(tx)) = (
                                *self.active_chan.lock().unwrap(),
                                self.frame_tx_cb.lock().unwrap().as_ref(),
                            )
                        {
                            *reqs -= 1;
                            let _ = tx.send(WebcamFrame {
                                data: output,
                                channel_ptr: chan,
                            });
                        }
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
                    let now = Instant::now();
                    next_frame_at = next_deadline(next_frame_at, interval, now);
                    thread::sleep(next_frame_at - now);
                } else {
                    thread::sleep(Duration::from_millis(50));
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reopen_waits_for_the_scheduled_instant() {
        let now = Instant::now();
        assert!(
            !reopen_due(None, now),
            "no failure pending, nothing to retry"
        );
        assert!(!reopen_due(Some(now + Duration::from_secs(1)), now));
        assert!(reopen_due(Some(now), now));
        assert!(reopen_due(Some(now - Duration::from_secs(1)), now));
    }

    #[test]
    fn deadline_advances_by_one_interval_when_work_is_fast() {
        let interval = Duration::from_millis(33);
        let previous = Instant::now();
        let now = previous + Duration::from_millis(10);
        assert_eq!(next_deadline(previous, interval, now), previous + interval);
    }

    #[test]
    fn deadline_does_not_accumulate_debt_when_work_is_slow() {
        let interval = Duration::from_millis(33);
        let previous = Instant::now();
        let now = previous + Duration::from_millis(500);
        assert_eq!(next_deadline(previous, interval, now), now);
    }
}
