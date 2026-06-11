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
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::thread;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use flume::{Receiver, Sender, unbounded};
use shared::log;

use super::tools::{ResamplerIterator, f32_to_pcm};

pub enum MicCommand {
    Stop,
}

pub struct MicHandle {
    pub tx: Sender<MicCommand>,
}

impl MicHandle {
    pub fn new(
        sample_rate: u32,
        channels: u16,
        bits_per_sample: u16,
        frames_per_packet: u32,
    ) -> (Self, Receiver<Vec<u8>>) {
        log::debug!(
            "Initializing mic: sample_rate={}, channels={}, bits_per_sample={}, frames_per_packet={}",
            sample_rate,
            channels,
            bits_per_sample,
            frames_per_packet
        );

        let (data_tx, data_rx) = unbounded::<Vec<u8>>();
        let (cmd_tx, cmd_rx) = unbounded::<MicCommand>();

        thread::spawn(move || {
            let host = cpal::default_host();
            let device = match host.default_input_device() {
                Some(d) => {
                    log::debug!("Default input device: {:?}", d.description());
                    d
                }
                None => {
                    log::error!("[MicHandle] No input device found");
                    return;
                }
            };

            let config = match get_input_config(&device, sample_rate, channels) {
                Some(c) => c,
                None => {
                    log::error!("[MicHandle] No suitable input config found");
                    return;
                }
            };

            let cfg = config.config();
            let actual_rate = cfg.sample_rate;
            let actual_channels = cfg.channels as usize;
            let need_resample = actual_rate != sample_rate;
            let need_downmix = actual_channels > channels as usize;

            log::debug!(
                "[MicHandle] Capture config: actual_rate={}, actual_channels={}, resample={}, downmix={}",
                actual_rate,
                actual_channels,
                need_resample,
                need_downmix
            );

            let out_packet_frames = (frames_per_packet as usize) * (channels as usize);
            let input_packet_frames = if need_resample {
                ((out_packet_frames as f32 * actual_rate as f32 / sample_rate as f32).ceil() as usize)
                    .max(out_packet_frames * 2)
            } else {
                out_packet_frames * if need_downmix { actual_channels } else { 1 }
            };

            let buffer: Arc<RwLock<VecDeque<f32>>> =
                Arc::new(RwLock::new(VecDeque::with_capacity(input_packet_frames * 4)));

            let stream = match device.build_input_stream(
                cfg,
                {
                    let data_tx = data_tx.clone();
                    let buffer = Arc::clone(&buffer);
                    move |data: &[f32], _| {
                        let mut buf = buffer.write().unwrap();
                        buf.extend(data.iter().copied());
                        while buf.len() >= input_packet_frames {
                            let chunk: Vec<f32> = buf.drain(0..input_packet_frames).collect();
                            drop(buf);

                            let out: Vec<f32> = if need_resample {
                                ResamplerIterator::new(chunk.into_iter(), actual_rate, sample_rate)
                                    .collect()
                            } else {
                                chunk
                            };

                            let out: Vec<f32> = if need_downmix {
                                let mut mono = Vec::with_capacity(out.len() / actual_channels);
                                for stereo_frame in out.chunks_exact(actual_channels) {
                                    mono.push(stereo_frame.iter().sum::<f32>() / actual_channels as f32);
                                }
                                mono
                            } else {
                                out
                            };

                            let pcm = f32_to_pcm(&out, bits_per_sample);
                            let _ = data_tx.send(pcm);
                            buf = buffer.write().unwrap();
                        }
                    }
                },
                |err| log::error!("[MicHandle] Input stream error: {}", err),
                None,
            ) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("[MicHandle] Failed to build input stream: {}", e);
                    return;
                }
            };

            if let Err(e) = stream.play() {
                log::error!("[MicHandle] Failed to start input stream: {}", e);
                return;
            }

            log::debug!("[MicHandle] Capture started");
            let _ = cmd_rx.recv();
            drop(stream);
            log::debug!("[MicHandle] Capture stopped");
        });

        (Self { tx: cmd_tx }, data_rx)
    }
}

fn get_input_config(
    dev: &cpal::Device,
    sample_rate: u32,
    channels: u16,
) -> Option<cpal::SupportedStreamConfig> {
    let configs = match dev.supported_input_configs() {
        Ok(c) => c,
        Err(e) => {
            log::error!("[MicHandle] Failed to get input configs: {}", e);
            return None;
        }
    };

    let all: Vec<_> = configs.collect();
    log::debug!(
        "[MicHandle] Supported configs: {:?}",
        all.iter()
            .map(|c| format!(
                "ch={}, rates=[{}..{}], fmt={:?}",
                c.channels(),
                c.min_sample_rate(),
                c.max_sample_rate(),
                c.sample_format()
            ))
            .collect::<Vec<_>>()
    );

    for cfg in &all {
        if cfg.channels() == channels
            && cfg.min_sample_rate() <= sample_rate
            && sample_rate <= cfg.max_sample_rate()
        {
            return Some(
                cfg.try_with_sample_rate(sample_rate)
                    .unwrap_or_else(|| cfg.with_max_sample_rate()),
            );
        }
    }

    log::warn!(
        "[MicHandle] No config for channels={} @ {}Hz, trying any channel",
        channels,
        sample_rate
    );
    for cfg in &all {
        if cfg.min_sample_rate() <= sample_rate
            && sample_rate <= cfg.max_sample_rate()
        {
            return Some(
                cfg.try_with_sample_rate(sample_rate)
                    .unwrap_or_else(|| cfg.with_max_sample_rate()),
            );
        }
    }

    log::warn!(
        "[MicHandle] No config for {}Hz, falling back to first available config",
        sample_rate
    );
    all.into_iter()
        .next()
        .map(|cfg| cfg.with_max_sample_rate())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mic_handle_creation() {
        let (handle, _data_rx) = MicHandle::new(44100, 1, 16, 480);
        assert!(handle.tx.send(MicCommand::Stop).is_ok());
    }
}
