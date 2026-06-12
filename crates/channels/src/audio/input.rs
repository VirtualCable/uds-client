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
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use flume::{Receiver, Sender, unbounded};
use shared::log;

use super::tools::{ResamplerIterator, f32_to_pcm};

use rdp::integrations::AudioInputIntegration;

pub enum MicCommand {
    Stop,
}

#[derive(Debug, Clone)]
pub struct MicHandle {
    pub tx: Arc<Mutex<Option<Sender<MicCommand>>>>,
}

impl MicHandle {
    pub fn new() -> Self {
        MicHandle {
            tx: Arc::new(Mutex::new(None)),
        }
    }
}

impl AudioInputIntegration for MicHandle {
    fn start(
        &self,
        sample_rate: u32,
        channels: u16,
        bits_per_sample: u16,
        frames_per_packet: u32,
    ) -> anyhow::Result<Receiver<Vec<u8>>> {
        log::debug!(
            "Initializing mic: sample_rate={}, channels={}, bits_per_sample={}, frames_per_packet={}",
            sample_rate,
            channels,
            bits_per_sample,
            frames_per_packet
        );
        self.stop();

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

            let out_packet_samples = (frames_per_packet as usize) * (channels as usize);

            let sample_buf: Arc<RwLock<VecDeque<f32>>> =
                Arc::new(RwLock::new(VecDeque::with_capacity(out_packet_samples * 4)));

            let stream = match device.build_input_stream(
                cfg,
                {
                    let data_tx = data_tx.clone();
                    let sample_buf = Arc::clone(&sample_buf);
                    move |data: &[f32], _| {
                        let mut buf = sample_buf.write().unwrap();
                        buf.extend(data.iter().copied());

                        let min_input = if need_resample || need_downmix {
                            (out_packet_samples * 2).max(256)
                        } else {
                            out_packet_samples
                        };

                        while buf.len() >= min_input {
                            let drain_end = buf.len().min(min_input * 2);
                            let chunk: Vec<f32> = buf.drain(0..drain_end).collect();
                            drop(buf);

                            let processed: Vec<f32> = if need_resample {
                                ResamplerIterator::new(chunk.into_iter(), actual_rate, sample_rate)
                                    .collect()
                            } else {
                                chunk
                            };

                            let processed: Vec<f32> = if need_downmix {
                                let mut mono =
                                    Vec::with_capacity(processed.len() / actual_channels);
                                for frame in processed.chunks_exact(actual_channels) {
                                    mono.push(frame.iter().sum::<f32>() / actual_channels as f32);
                                }
                                mono
                            } else {
                                processed
                            };

                            for packet in processed.chunks(out_packet_samples) {
                                let mut pkt = packet.to_vec();
                                pkt.resize(out_packet_samples, 0.0);
                                let pcm = f32_to_pcm(&pkt, bits_per_sample);
                                let _ = data_tx.send(pcm);
                            }
                            buf = sample_buf.write().unwrap();
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

        *self.tx.lock().unwrap() = Some(cmd_tx);
        Ok(data_rx)
    }

    fn stop(&self) {
        let mut tx_guard = self.tx.lock().unwrap();
        if let Some(tx) = tx_guard.take() {
            let _ = tx.send(MicCommand::Stop);
        }
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
        if cfg.min_sample_rate() <= sample_rate && sample_rate <= cfg.max_sample_rate() {
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
    all.into_iter().next().map(|cfg| cfg.with_max_sample_rate())
}

impl Default for MicHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mic_handle_creation() {
        let handle = MicHandle::new();
        let _rx = handle.start(44100, 1, 16, 480);
        handle.stop();
    }
}
