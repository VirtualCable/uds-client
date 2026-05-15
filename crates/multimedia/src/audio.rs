// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
//
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use flume::{Sender, unbounded};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::thread;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use shared::log;

pub enum AudioCommand {
    Play(Vec<u8>),
    SetVolume(u32),
    Close,
}

pub struct AudioHandle {
    pub tx: Sender<AudioCommand>,
    pub volume: Arc<RwLock<u32>>,
    pub latency: Arc<RwLock<u32>>,
}

impl AudioHandle {
    pub fn new(
        n_channels: u16,
        sample_rate: u32,
        bits_per_sample: u16,
        latency_threshold: Option<u16>,
    ) -> Self {
        log::debug!(
            "Initializing audio: channels={}, sample_rate={}, bits_per_sample={}, latency_cushion={:?}",
            n_channels,
            sample_rate,
            bits_per_sample,
            latency_threshold
        );
        let (tx, rx) = unbounded::<AudioCommand>();
        let volume = Arc::new(RwLock::new(0xFFFFFFFF));
        let latency = Arc::new(RwLock::new(190)); // Expected initial RDP latency for a single buffer

        thread::spawn({
            let volume = Arc::clone(&volume);
            let latency = Arc::clone(&latency);
            let latency_threshold = latency_threshold.map(|lt| (lt as f32).clamp(300.0, 1000.0));

            move || {
                let host = cpal::default_host();
                let device = host.default_output_device();
                log::debug!(
                    "Default audio output device: {:?}",
                    device.as_ref().map(|d| d.description())
                );
                let mut stream = None;

                // Shared buffer for audio samples
                let buffer: Arc<RwLock<VecDeque<f32>>> = Arc::new(RwLock::new(VecDeque::new()));

                // Keep last 32 play requests stamp, to calculate the mean time between them
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
                            &cfg,
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
                    s.play().unwrap();
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
                                    // This is so beceuse we use iterators and we don't know the length in advance
                                    let added_frames = {
                                        let buf_len = buf.len();
                                        buf.extend(resampled_iter);
                                        (buf.len() - buf_len) as u64 / n_channels as u64
                                    };
                                    stats.add_frames_played(added_frames);

                                    // Update approximate latency
                                    let frames = buf.len() as u32 / n_channels as u32;
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
                                            * n_channels as usize;
                                        if buf.len() > target_frames {
                                            let drop = buf.len() - target_frames;
                                            stats.add_frames_dropped(
                                                (drop / n_channels as usize) as u64,
                                            );
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
            }
        });

        AudioHandle {
            tx,
            volume,
            latency,
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

    pub fn play(&self, data: Vec<u8>) -> Result<(), flume::SendError<AudioCommand>> {
        self.tx.send(AudioCommand::Play(data))
    }
}

// Resampling iterator to adjust sample rates
pub struct ResamplerIterator<I> {
    inner: I,
    input_rate: f32,
    output_rate: f32,
    buffer: Vec<f32>,
    pos: f32,
    passthrough: bool,
}

impl<I: Iterator<Item = f32>> ResamplerIterator<I> {
    pub fn new(inner: I, input_rate: u32, output_rate: u32) -> Self {
        Self {
            inner,
            input_rate: input_rate as f32,
            output_rate: output_rate as f32,
            buffer: Vec::new(),
            pos: 0.0,
            passthrough: input_rate == output_rate,
        }
    }
}

impl<I: Iterator<Item = f32>> Iterator for ResamplerIterator<I> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.passthrough {
            return self.inner.next();
        }
        // sample rate ratio
        let ratio = self.input_rate / self.output_rate;

        // fill buffer if needed
        if self.buffer.len() < 2 {
            if let Some(sample) = self.inner.next() {
                self.buffer.push(sample);
            } else {
                return None;
            }
        }

        // simple linear interpolation
        let i = self.pos.floor() as usize;
        let frac = self.pos - i as f32;

        // while i + 1 >= self.buffer.len() { // Original line was wrong, it should be while...
        //     // load more data
        //     if let Some(sample) = self.inner.next() {
        //         self.buffer.push(sample);
        //     } else {
        //         return None;
        //     }
        // }

        // Corrected logic for interpolation boundary check
        while i + 1 >= self.buffer.len() {
            // load more data
            if let Some(sample) = self.inner.next() {
                self.buffer.push(sample);
            } else {
                return None;
            }
        }

        let s0 = self.buffer[i];
        let s1 = self.buffer[i + 1];
        let out = s0 + (s1 - s0) * frac;

        // next sample position
        self.pos += ratio;

        // clear buffer of consumed samples
        while self.pos >= 1.0 {
            self.pos -= 1.0;
            if !self.buffer.is_empty() {
                self.buffer.remove(0);
            }
        }

        Some(out)
    }
}

fn pcm_to_f32<'a>(data: &'a [u8], bits_per_sample: u16) -> impl Iterator<Item = f32> + 'a {
    match bits_per_sample {
        8 => Box::new(data.iter().map(|&b| (b as i8) as f32 / i8::MAX as f32))
            as Box<dyn Iterator<Item = f32>>,
        16 => Box::new(
            data.chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / i16::MAX as f32),
        ) as Box<dyn Iterator<Item = f32>>,
        24 => Box::new(data.chunks_exact(3).map(|c| {
            let v = ((c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16)) << 8;
            v as f32 / i32::MAX as f32
        })) as Box<dyn Iterator<Item = f32>>,
        32 => Box::new(
            data.chunks_exact(4)
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / i32::MAX as f32),
        ) as Box<dyn Iterator<Item = f32>>,
        _ => Box::new(std::iter::empty()) as Box<dyn Iterator<Item = f32>>,
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

    /// Notes:
    /// On first calls, this value may be inaccurate due to lack of data,
    /// so we will return 180.0 ms as a default value that is a bit less than initial expected RDP call rate.
    /// After that, it will be the actual mean interval.
    /// If sound is paused this value will grow a lot for the last calls, but it's acceptable
    /// in our use case, where we only use it to adjust "drift" on playback when latency is too high
    /// compared with this value.
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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::log;

    // The shared::log needs to be imported here because it was available in the original lib.rs scope,
    // and tests use it. If I don't import it here, the tests will fail.

    #[test]
    fn test_audio_handle_creation() {
        let handle = AudioHandle::new(2, 44100, 16, None);
        assert!(handle.tx.send(AudioCommand::Close).is_ok());
    }

    #[test]
    fn test_audio_play_command() {
        log::setup_logging("debug", log::LogType::Test);
        let handle = AudioHandle::new(2, 44100, 16, None);
        *handle.latency.write().unwrap() = 8888; // set initial latency for later check
        let sample_data = vec![0u8; 44100 * 2 * 2]; // 1 second of silence in 16-bit stereo
        assert!(handle.tx.send(AudioCommand::Play(sample_data)).is_ok());
        let start = std::time::Instant::now();
        // Show latency after sending play command
        for _ in 0..10 {
            let latency_val = *handle.latency.read().unwrap();
            if latency_val != 8888 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let latency = *handle.latency.read().unwrap();
        log::debug!(
            "Approximate latency after play command: {} ms ({} elapsed)",
            latency,
            start.elapsed().as_millis()
        );
        // wait 1 second, send 100 ms of audio at a time and check latency again
        *handle.latency.write().unwrap() = 8888;
        handle
            .tx
            .send(AudioCommand::Play(vec![0u8; 44100 * 2 * 2 / 10]))
            .unwrap();
        for _ in 0..10 {
            let latency_val = *handle.latency.read().unwrap();
            if latency_val != 8888 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let latency = *handle.latency.read().unwrap();
        log::debug!(
            "Approximate latency after incremental play commands: {} ms ({} elapsed)",
            latency,
            start.elapsed().as_millis()
        );
        assert!(handle.tx.send(AudioCommand::Close).is_ok());
    }

    // ── Pure unit tests ────────────────────────────────────

    #[test]
    fn pcm_8bit() {
        // 8-bit: (b as i8) as f32 / i8::MAX (127.0)
        // 0 → 0.0, 127 (as i8 = 127) → ~1.0, 255 (as i8 = -1) → ~-0.00787
        let data: Vec<f32> = pcm_to_f32(&[0, 127, 255], 8).collect();
        assert!(data[0].abs() < 0.01);
        assert!((data[1] - 1.0).abs() < 0.01);
        assert!(data[2] < 0.0 && data[2] > -0.02);
    }

    #[test]
    fn pcm_16bit() {
        // i16::MAX = 32767
        let max: i16 = i16::MAX;
        let data: Vec<f32> = pcm_to_f32(&max.to_le_bytes(), 16).collect();
        assert!((data[0] - 1.0).abs() < 0.001);
        let zero: Vec<f32> = pcm_to_f32(&0i16.to_le_bytes(), 16).collect();
        assert!(zero[0].abs() < 0.001);
    }

    #[test]
    fn pcm_16bit_stereo() {
        let samples: Vec<u8> = [100i16.to_le_bytes(), (-100i16).to_le_bytes()].concat();
        let data: Vec<f32> = pcm_to_f32(&samples, 16).collect();
        assert_eq!(data.len(), 2);
        assert!(data[0] > 0.0);
        assert!(data[1] < 0.0);
    }

    #[test]
    fn pcm_24bit_zero() {
        let data: Vec<f32> = pcm_to_f32(&[0u8; 6], 24).collect();
        assert_eq!(data.len(), 2);
        assert!(data[0].abs() < 0.001);
        assert!(data[1].abs() < 0.001);
    }

    #[test]
    fn pcm_32bit() {
        let data: Vec<f32> = pcm_to_f32(&0i32.to_le_bytes(), 32).collect();
        assert!(data[0].abs() < 0.001);
    }

    #[test]
    fn pcm_unknown_bits_returns_empty() {
        let data: Vec<f32> = pcm_to_f32(&[1, 2, 3, 4], 5).collect();
        assert!(data.is_empty());
    }

    #[test]
    fn pcm_empty_input() {
        assert!(pcm_to_f32(&[], 8).next().is_none());
        assert!(pcm_to_f32(&[], 16).next().is_none());
    }

    #[test]
    fn resampler_passthrough() {
        let input = vec![0.1, 0.5, -0.3];
        let resampler = ResamplerIterator::new(input.clone().into_iter(), 44100, 44100);
        let output: Vec<f32> = resampler.collect();
        assert_eq!(output.len(), 3);
        for (i, v) in output.iter().enumerate() {
            assert!((v - input[i]).abs() < 0.001);
        }
    }

    #[test]
    fn resampler_upsample_2x() {
        let input = vec![0.0, 1.0];
        let resampler = ResamplerIterator::new(input.into_iter(), 24000, 48000);
        let output: Vec<f32> = resampler.collect();
        assert!(output.len() >= 2, "upsampling should produce more samples");
    }

    #[test]
    fn resampler_downsample_2x() {
        let input: Vec<f32> = (0..8).map(|i| i as f32).collect();
        let in_len = input.len();
        let resampler = ResamplerIterator::new(input.into_iter(), 48000, 24000);
        let output: Vec<f32> = resampler.collect();
        assert!(
            output.len() < in_len,
            "downsampling should produce fewer samples"
        );
    }

    #[test]
    fn resampler_empty() {
        let resampler = ResamplerIterator::new(std::iter::empty::<f32>(), 44100, 48000);
        assert_eq!(resampler.count(), 0);
    }

    #[test]
    fn resampler_new_passthrough_flag() {
        let r = ResamplerIterator::new(std::iter::empty::<f32>(), 44100, 44100);
        assert!(r.passthrough);
        let r = ResamplerIterator::new(std::iter::empty::<f32>(), 44100, 48000);
        assert!(!r.passthrough);
    }

    #[test]
    fn audio_stats_new_defaults() {
        let s = AudioStats::new();
        assert_eq!(s.total_play_calls, 0);
        assert_eq!(s.total_frames_played, 0);
        assert_eq!(s.total_frames_dropped, 0);
        assert!(s.last_call.is_none());
        assert!(s.time_between_play_calls.is_empty());
    }

    #[test]
    fn audio_stats_add_frames_played() {
        let mut s = AudioStats::new();
        s.add_frames_played(100);
        assert_eq!(s.total_frames_played, 100);
        s.add_frames_played(50);
        assert_eq!(s.total_frames_played, 150);
        s.add_frames_played(0);
        assert_eq!(s.total_frames_played, 150);
    }

    #[test]
    fn audio_stats_add_frames_dropped() {
        let mut s = AudioStats::new();
        s.add_frames_dropped(10);
        assert_eq!(s.total_frames_dropped, 10);
        s.add_frames_dropped(5);
        assert_eq!(s.total_frames_dropped, 15);
    }

    #[test]
    fn audio_stats_mean_empty_returns_default() {
        let s = AudioStats::new();
        assert!((s.mean_calls_interval() - 180.0).abs() < 0.01);
    }

    #[test]
    fn audio_stats_mean_with_values() {
        let mut s = AudioStats::new();
        // Manually push a value below MIN_EXPECTED_INTERVAL (180ms)
        s.time_between_play_calls.push_back(100);
        s.time_between_play_calls.push_back(200);
        // mean = 150, clamped to 180.0
        assert!((s.mean_calls_interval() - 180.0).abs() < 0.01);
    }

    #[test]
    fn audio_stats_mean_no_clamp() {
        let mut s = AudioStats::new();
        s.time_between_play_calls.push_back(300);
        s.time_between_play_calls.push_back(500);
        // mean = 400, above MIN
        assert!((s.mean_calls_interval() - 400.0).abs() < 0.01);
    }

    #[test]
    fn audio_stats_add_play_call_first_time() {
        let mut s = AudioStats::new();
        s.add_play_call();
        assert!(s.last_call.is_some());
        assert_eq!(s.total_play_calls, 0); // first call doesn't count
        assert!(s.time_between_play_calls.is_empty());
    }

    #[test]
    fn audio_stats_add_play_call_second_time() {
        let mut s = AudioStats::new();
        s.add_play_call();
        s.add_play_call();
        assert_eq!(s.total_play_calls, 1);
        assert_eq!(s.time_between_play_calls.len(), 1);
    }
}
