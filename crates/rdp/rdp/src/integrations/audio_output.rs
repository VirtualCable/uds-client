// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.

pub trait AudioOutputIntegration: Send + Sync + std::fmt::Debug {
    fn open(
        &self,
        channels: u16,
        sample_rate: u32,
        bits_per_sample: u16,
        latency_threshold: Option<u32>,
    );
    fn play(&self, data: &[u8]) -> u32; // Returns current latency in ms
    fn get_volume(&self) -> u32;
    fn set_volume(&self, volume: u32);
    fn close(&self);
}
