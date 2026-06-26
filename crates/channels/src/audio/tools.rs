// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

use shared::log;

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
        let ratio = self.input_rate / self.output_rate;

        if self.buffer.len() < 2 {
            if let Some(sample) = self.inner.next() {
                self.buffer.push(sample);
            } else {
                return None;
            }
        }

        let i = self.pos.floor() as usize;
        let frac = self.pos - i as f32;

        while i + 1 >= self.buffer.len() {
            if let Some(sample) = self.inner.next() {
                self.buffer.push(sample);
            } else {
                return None;
            }
        }

        let s0 = self.buffer[i];
        let s1 = self.buffer[i + 1];
        let out = s0 + (s1 - s0) * frac;

        self.pos += ratio;

        while self.pos >= 1.0 {
            self.pos -= 1.0;
            if !self.buffer.is_empty() {
                self.buffer.remove(0);
            }
        }

        Some(out)
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

    #[test]
    fn test_passthrough() {
        let input = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let resampler = ResamplerIterator::new(input.clone().into_iter(), 44100, 44100);
        let out: Vec<f32> = resampler.collect();
        assert_eq!(out.len(), input.len());
        for (a, b) in out.iter().zip(input.iter()) {
            assert!((a - b).abs() < 0.0001);
        }
    }

    #[test]
    fn test_upsample() {
        let input = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let in_len = input.len();
        let resampler = ResamplerIterator::new(input.into_iter(), 24000, 48000);
        let out: Vec<f32> = resampler.collect();
        assert!(out.len() > in_len);
    }

    #[test]
    fn test_downsample() {
        let input = vec![0.0, 0.5, -0.5, 1.0, -1.0];
        let in_len = input.len();
        let resampler = ResamplerIterator::new(input.into_iter(), 48000, 24000);
        let out: Vec<f32> = resampler.collect();
        assert!(out.len() < in_len);
    }

    #[test]
    fn test_empty_passthrough() {
        let r = ResamplerIterator::new(std::iter::empty::<f32>(), 44100, 44100);
        assert_eq!(r.count(), 0);
    }

    #[test]
    fn test_empty_resample() {
        let r = ResamplerIterator::new(std::iter::empty::<f32>(), 44100, 48000);
        assert_eq!(r.count(), 0);
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
