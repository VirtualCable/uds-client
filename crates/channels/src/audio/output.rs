// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use flume::{Sender, unbounded};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use shared::log;

use super::tools::{ResamplerIterator, pcm_to_f32};

use rdp::integrations::AudioOutputIntegration;

#[derive(Debug)]
pub enum AudioCommand {
    Play(Vec<u8>),
    SetVolume(u32),
    Close,
}

#[derive(Debug, Clone)]
pub struct AudioHandle {
    pub tx: Arc<Mutex<Option<Sender<AudioCommand>>>>,
    pub volume: Arc<RwLock<u32>>,
    pub latency: Arc<RwLock<u32>>,
}

impl AudioHandle {
    pub fn new() -> Self {
        AudioHandle {
            tx: Arc::new(Mutex::new(None)),
            volume: Arc::new(RwLock::new(0xFFFFFFFF)),
            latency: Arc::new(RwLock::new(190)),
        }
    }

    pub fn get_stream_config(
        dev: &cpal::Device,
        sample_rate: u32,
    ) -> Option<cpal::SupportedStreamConfig> {
        if let Ok(configs) = dev.supported_output_configs() {
            for cfg in configs {
                if cfg.min_sample_rate() <= sample_rate && sample_rate <= cfg.max_sample_rate() {
                    return Some(
                        cfg.try_with_sample_rate(sample_rate)
                            .unwrap_or(cfg.with_max_sample_rate()),
                    );
                }
            }
        }
        None
    }
}

impl AudioOutputIntegration for AudioHandle {
    fn open(
        &self,
        channels: u16,
        sample_rate: u32,
        bits_per_sample: u16,
        latency_threshold: Option<u32>,
    ) {
        log::debug!(
            "Initializing audio: channels={}, sample_rate={}, bits_per_sample={}, latency_cushion={:?}",
            channels,
            sample_rate,
            bits_per_sample,
            latency_threshold
        );
        self.close();
        let (tx, rx) = unbounded::<AudioCommand>();

        let volume = Arc::clone(&self.volume);
        let latency = Arc::clone(&self.latency);
        let latency_threshold = latency_threshold.map(|lt| (lt as f32).clamp(300.0, 1000.0));

        thread::spawn(move || {
            let host = cpal::default_host();
            let device = host.default_output_device();
            log::debug!(
                "Default audio output device: {:?}",
                device.as_ref().map(|d| d.description())
            );
            let mut stream = None;

            // Shared buffer for audio samples
            let buffer: Arc<RwLock<VecDeque<f32>>> = Arc::new(RwLock::new(VecDeque::new()));

            let mut output_sample_rate = sample_rate;
            if let Some(dev) = device
                && let Some(cfg) = AudioHandle::get_stream_config(&dev, sample_rate)
            {
                log::debug!(
                    "Using audio format: {:?}, range={}",
                    cfg.sample_format(),
                    cfg.sample_rate()
                );
                let cfg = cfg.config();
                // Store real output sample rate
                output_sample_rate = cfg.sample_rate;
                stream = Some(
                    dev.build_output_stream(
                        cfg,
                        {
                            let buffer = Arc::clone(&buffer);
                            move |data: &mut [f32], _| {
                                let mut buf_guard = buffer.write().unwrap();
                                for sample in data.iter_mut() {
                                    if let Some(val) = buf_guard.pop_front() {
                                        *sample = val;
                                    } else {
                                        *sample = 0.0; // No more data, output silence
                                    }
                                }
                            }
                        },
                        move |err| log::error!("Stream error: {}", err),
                        None,
                    )
                    .unwrap(),
                );
            }

            if let Some(s) = &stream {
                let _ = s.play();
            } else {
                log::error!("Audio disabled: cpal init failed");
            }

            let mut stats = AudioStats::new();
            // Main loop
            loop {
                if let Ok(cmd) = rx.recv() {
                    match cmd {
                        AudioCommand::Play(data) => {
                            stats.add_play_call();
                            if stream.is_some() {
                                // Convert PCM to f32, resample and push to buffer
                                let resampled_iter = ResamplerIterator::new(
                                    pcm_to_f32(&data, bits_per_sample),
                                    sample_rate,
                                    output_sample_rate,
                                );
                                let mut buf = buffer.write().unwrap();
                                // Store current buffer length to calculate number of frames added
                                let added_frames = {
                                    let buf_len = buf.len();
                                    buf.extend(resampled_iter);
                                    (buf.len() - buf_len) as u64 / channels as u64
                                };
                                stats.add_frames_played(added_frames);

                                // Update approximate latency
                                let frames = buf.len() as u32 / channels as u32;
                                let ms = (frames as f32 / output_sample_rate as f32) * 1000.0;
                                *latency.write().unwrap() = ms as u32;

                                let latency_threshold = match latency_threshold {
                                    Some(lt) => lt,
                                    None => stats.mean_calls_interval() * 2.0, // default to double the mean interval
                                };
                                // overflow control: if latency > latency_threshold ms, drop some frames
                                if ms > latency_threshold {
                                    // try to get back to ~200 ms latency
                                    let target_frames = ((stats.mean_calls_interval() / 1000.0)
                                        * output_sample_rate as f32)
                                        as usize
                                        * channels as usize;
                                    if buf.len() > target_frames {
                                        let drop = buf.len() - target_frames;
                                        stats.add_frames_dropped((drop / channels as usize) as u64);
                                        buf.drain(0..drop);
                                        log::warn!(
                                            "Dropped {} frames to recover sync, new latency ~{} ms",
                                            drop,
                                            200
                                        );
                                        *latency.write().unwrap() = 200; // Proximate latency after drop
                                    }
                                }
                            }
                        }
                        AudioCommand::SetVolume(v) => {
                            *volume.write().unwrap() = v;
                        }
                        AudioCommand::Close => break,
                    }
                }
            }
        });

        *self.tx.lock().unwrap() = Some(tx);
    }

    fn play(&self, data: &[u8]) -> u32 {
        if let Some(ref tx) = *self.tx.lock().unwrap() {
            let _ = tx.send(AudioCommand::Play(data.to_vec()));
        }
        *self.latency.read().unwrap()
    }

    fn get_volume(&self) -> u32 {
        *self.volume.read().unwrap()
    }

    fn set_volume(&self, volume: u32) {
        *self.volume.write().unwrap() = volume;
        if let Some(ref tx) = *self.tx.lock().unwrap() {
            let _ = tx.send(AudioCommand::SetVolume(volume));
        }
    }

    fn close(&self) {
        let mut tx_guard = self.tx.lock().unwrap();
        if let Some(tx) = tx_guard.take() {
            let _ = tx.send(AudioCommand::Close);
        }
    }
}

// Keep statistics about audio playback
// Such as time between play calls, dropped frames, etc
pub struct AudioStats {
    last_call: Option<std::time::Instant>,
    pub total_play_calls: u64,
    pub total_frames_played: u64,
    pub total_frames_dropped: u64,
    // keep last 32 time between play calls to calculate average
    pub time_between_play_calls: VecDeque<u128>, // time between play calls in ms
}

#[allow(clippy::new_without_default)]
impl AudioStats {
    pub fn new() -> Self {
        AudioStats {
            last_call: None,
            total_play_calls: 0,
            total_frames_played: 0,
            total_frames_dropped: 0,
            time_between_play_calls: VecDeque::with_capacity(32),
        }
    }

    pub fn add_play_call(&mut self) {
        let last_call = match self.last_call {
            Some(t) => t,
            None => {
                self.last_call = Some(std::time::Instant::now());
                return;
            }
        };
        self.last_call = Some(std::time::Instant::now());
        self.total_play_calls += 1;
        let now = std::time::Instant::now();
        let duration = now.duration_since(last_call).as_millis();
        if self.time_between_play_calls.len() == 32 {
            self.time_between_play_calls.pop_front();
        }
        self.time_between_play_calls.push_back(duration);
    }

    pub fn mean_calls_interval(&self) -> f32 {
        const MIN_EXPECTED_INTERVAL: f32 = 180.0;
        if self.time_between_play_calls.is_empty() {
            return MIN_EXPECTED_INTERVAL;
        }
        let sum: u128 = self.time_between_play_calls.iter().sum();
        (sum as f32 / self.time_between_play_calls.len() as f32).max(MIN_EXPECTED_INTERVAL) // minimum expected interval
    }

    pub fn add_frames_played(&mut self, frames: u64) {
        self.total_frames_played += frames;
    }

    pub fn add_frames_dropped(&mut self, frames: u64) {
        self.total_frames_dropped += frames;
    }
}

impl Default for AudioHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_handle_creation() {
        let handle = AudioHandle::new();
        handle.open(2, 44100, 16, None);
        handle.close();
    }
}
