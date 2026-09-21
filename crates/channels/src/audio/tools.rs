// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use shared::log;

/// Linear resampler for interleaved audio. It works in frames, so channels are never
/// mixed, and keeps the last input frame between calls so consecutive packets join
/// without a discontinuity.
pub struct Resampler {
    channels: usize,
    step: f64,
    pos: f64,
    prev: Option<Vec<f32>>,
    passthrough: bool,
}

impl Resampler {
    pub fn new(input_rate: u32, output_rate: u32, channels: u16) -> Self {
        Self {
            channels: channels.max(1) as usize,
            step: input_rate as f64 / output_rate as f64,
            pos: 0.0,
            prev: None,
            passthrough: input_rate == output_rate,
        }
    }

    pub fn process(&mut self, input: &[f32], out: &mut impl Extend<f32>) {
        if self.passthrough {
            out.extend(input.iter().copied());
            return;
        }
        let frames: Vec<&[f32]> = self
            .prev
            .as_deref()
            .into_iter()
            .chain(input.chunks_exact(self.channels))
            .collect();
        if frames.len() < 2 {
            self.prev = frames.first().map(|f| f.to_vec());
            return;
        }

        while (self.pos as usize) + 1 < frames.len() {
            let i = self.pos as usize;
            let frac = (self.pos - i as f64) as f32;
            let (a, b) = (frames[i], frames[i + 1]);
            out.extend((0..self.channels).map(|c| a[c] + (b[c] - a[c]) * frac));
            self.pos += self.step;
        }

        self.pos -= (frames.len() - 1) as f64;
        self.prev = frames.last().map(|f| f.to_vec());
    }
}

pub fn pcm_to_f32<'a>(data: &'a [u8], bits_per_sample: u16) -> impl Iterator<Item = f32> + 'a {
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

pub fn f32_to_pcm(data: &[f32], bits_per_sample: u16) -> Vec<u8> {
    match bits_per_sample {
        8 => data
            .iter()
            .map(|&s| {
                let clamped = s.clamp(-1.0, 1.0);
                (clamped * i8::MAX as f32) as i8 as u8
            })
            .collect(),
        16 => {
            let mut out = Vec::with_capacity(data.len() * 2);
            for &s in data {
                let clamped = s.clamp(-1.0, 1.0);
                let v = (clamped * i16::MAX as f32) as i16;
                out.extend_from_slice(&v.to_le_bytes());
            }
            out
        }
        _ => {
            log::error!(
                "[audio_tools] Unsupported bits per sample: {}",
                bits_per_sample
            );
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resample(input: &[f32], from: u32, to: u32, channels: u16) -> Vec<f32> {
        let mut out = Vec::new();
        Resampler::new(from, to, channels).process(input, &mut out);
        out
    }

    #[test]
    fn passthrough_is_identity() {
        let input = vec![0.0, 0.5, -0.5, 1.0, -1.0, 0.25];
        assert_eq!(resample(&input, 44100, 44100, 2), input);
    }

    #[test]
    fn stereo_channels_are_not_mixed() {
        // Left constant 1.0, right constant -1.0: any cross-channel interpolation shows up.
        let input: Vec<f32> = (0..4410).flat_map(|_| [1.0, -1.0]).collect();
        let out = resample(&input, 44100, 48000, 2);
        for frame in out.as_chunks::<2>().0 {
            assert_eq!(*frame, [1.0, -1.0]);
        }
    }

    #[test]
    fn output_length_follows_rate_ratio() {
        let input = vec![0.0; 44100 * 2];
        let frames = resample(&input, 44100, 48000, 2).len() / 2;
        assert!((47998..=48001).contains(&frames), "got {frames} frames");
    }

    #[test]
    fn chunked_input_matches_single_call() {
        let input: Vec<f32> = (0..2000).map(|i| (i as f32 * 0.01).sin()).collect();
        let whole = resample(&input, 44100, 48000, 2);

        let mut resampler = Resampler::new(44100, 48000, 2);
        let mut chunked = Vec::new();
        for chunk in input.chunks(2 * 37) {
            resampler.process(chunk, &mut chunked);
        }
        assert_eq!(chunked.len(), whole.len());
        for (a, b) in chunked.iter().zip(&whole) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn downsample_interpolates_between_frames() {
        let input = vec![0.0, 1.0, 2.0, 3.0];
        assert_eq!(resample(&input, 48000, 24000, 1), vec![0.0, 2.0]);
    }

    #[test]
    fn empty_input_produces_nothing() {
        assert!(resample(&[], 44100, 48000, 2).is_empty());
        assert!(resample(&[], 44100, 44100, 2).is_empty());
    }

    #[test]
    fn pcm_8bit() {
        let data: Vec<f32> = pcm_to_f32(&[0, 127, 255], 8).collect();
        assert!(data[0].abs() < 0.01);
        assert!((data[1] - 1.0).abs() < 0.01);
        assert!(data[2] < 0.0 && data[2] > -0.02);
    }

    #[test]
    fn pcm_16bit() {
        let max: i16 = i16::MAX;
        let data: Vec<f32> = pcm_to_f32(&max.to_le_bytes(), 16).collect();
        assert!((data[0] - 1.0).abs() < 0.001);
        let zero: Vec<f32> = pcm_to_f32(&0i16.to_le_bytes(), 16).collect();
        assert!(zero[0].abs() < 0.001);
    }

    #[test]
    fn f32_to_pcm_16bit() {
        let samples = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let pcm = f32_to_pcm(&samples, 16);
        assert_eq!(pcm.len(), 10);
        assert_eq!(pcm[0..2], [0, 0]);
    }

    #[test]
    fn f32_to_pcm_8bit() {
        let samples = vec![0.0, 0.5, -0.5];
        let pcm = f32_to_pcm(&samples, 8);
        assert_eq!(pcm.len(), 3);
        assert_eq!(pcm[0], 0);
    }

    #[test]
    fn pcm_roundtrip_16bit() {
        let original: Vec<f32> = vec![0.0, 0.5, -0.5, 0.999];
        let pcm = f32_to_pcm(&original, 16);
        let recovered: Vec<f32> = pcm_to_f32(&pcm, 16).collect();
        assert_eq!(recovered.len(), original.len());
        for (a, b) in recovered.iter().zip(original.iter()) {
            assert!((a - b).abs() < 0.001);
        }
    }
}
