use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use crossbeam::channel::{Sender, unbounded};
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
    pub fn new(n_channels: u16, sample_rate: u32, bits_per_sample: u16) -> Self {
        let (tx, rx) = unbounded::<AudioCommand>();
        let volume = Arc::new(RwLock::new(0xFFFFFFFF));
        let latency = Arc::new(RwLock::new(0));

        thread::spawn({
            let volume = Arc::clone(&volume);
            let latency = Arc::clone(&latency);

            move || {
                let host = cpal::default_host();
                let device = host.default_output_device();
                let mut stream = None;

                // Buffer compartido para samples
                let buffer: Arc<RwLock<VecDeque<f32>>> = Arc::new(RwLock::new(VecDeque::new()));
                let buffer_cb = Arc::clone(&buffer);

                if let Some(dev) = device
                    && let Ok(mut configs) = dev.supported_output_configs()
                    && let Some(range) = configs.next()
                {
                    let cfg = range
                        .with_sample_rate(cpal::SampleRate(sample_rate))
                        .config();
                    stream = Some(
                        dev.build_output_stream(
                            &cfg,
                            move |data: &mut [f32], _| {
                                let mut buf = buffer_cb.write().unwrap();
                                for sample in data.iter_mut() {
                                    if let Some(val) = buf.pop_front() {
                                        *sample = val;
                                    } else {
                                        *sample = 0.0; // No more data, output silence
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

                // Main loop
                loop {
                    if let Ok(cmd) = rx.recv() {
                        match cmd {
                            AudioCommand::Play(data) => {
                                if stream.is_some() {
                                    // Convert PCM to f32 and push to buffer
                                    let mut buf = buffer.write().unwrap();
                                    buf.extend(pcm_to_f32(&data, bits_per_sample));

                                    // Update approximate latency
                                    let frames = buf.len() as u32 / n_channels as u32;
                                    let ms = (frames as f32 / sample_rate as f32) * 1000.0;
                                    *latency.write().unwrap() = ms as u32;
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

fn pcm_to_f32<'a>(data: &'a [u8], bits_per_sample: u16) -> impl Iterator<Item = f32> + 'a {
    match bits_per_sample {
        8 => Box::new(data.iter().map(|&b| (b as i8) as f32 / i8::MAX as f32)) as Box<dyn Iterator<Item = f32>>,
        16 => Box::new(
            data.chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / i16::MAX as f32),
        ) as Box<dyn Iterator<Item = f32>>,
        24 => Box::new(
            data.chunks_exact(3).map(|c| {
                let v = ((c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16)) << 8;
                v as f32 / i32::MAX as f32
            }),
        ) as Box<dyn Iterator<Item = f32>>,
        32 => Box::new(
            data.chunks_exact(4)
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / i32::MAX as f32),
        ) as Box<dyn Iterator<Item = f32>>,
        _ => Box::new(std::iter::empty()) as Box<dyn Iterator<Item = f32>>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_handle_creation() {
        let handle = AudioHandle::new(2, 44100, 16);
        assert!(handle.tx.send(AudioCommand::Close).is_ok());
    }

    #[test]
    fn test_audio_play_command() {
        log::setup_logging("debug", log::LogType::Tests);
        let handle = AudioHandle::new(2, 44100, 16);
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
