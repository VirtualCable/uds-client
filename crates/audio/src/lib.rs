// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
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
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use crossbeam::channel::{Sender, unbounded};
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
                let mut stream = None;

                // Shared buffer for audio samples
                let buffer: Arc<RwLock<VecDeque<f32>>> = Arc::new(RwLock::new(VecDeque::new()));

                // Keep last 32 play requests stamp, to calculate the mean time between them

                let mut output_sample_rate = sample_rate;
                if let Some(dev) = device
                    && let Ok(mut configs) = dev.supported_output_configs()
                    && let Some(range) = configs.next()
                {
                    log::debug!("Using audio format: {:?}, range={}", range, sample_rate);
                    let cfg = range
                        .try_with_sample_rate(cpal::SampleRate(sample_rate))
                        .unwrap_or(range.with_max_sample_rate())
                        .config();
                    // Store real output sample rate
                    output_sample_rate = cfg.sample_rate.0;
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

    pub fn play(&self, data: Vec<u8>) -> Result<(), crossbeam::channel::SendError<AudioCommand>> {
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

        if i + 1 >= self.buffer.len() {
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

        // clear buffer if consumed
        if self.pos >= 1.0 {
            self.pos -= 1.0;
            self.buffer.remove(0);
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

    #[test]
    fn test_audio_handle_creation() {
        let handle = AudioHandle::new(2, 44100, 16, None);
        assert!(handle.tx.send(AudioCommand::Close).is_ok());
    }

    #[test]
    fn test_audio_play_command() {
        log::setup_logging("debug", log::LogType::Tests);
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
}
