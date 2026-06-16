// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.

pub trait AudioInputIntegration: Send + Sync + std::fmt::Debug {
    /// Starts capturing audio and returns a flume Receiver where RDP will consume recorded frames.
    fn start(
        &self,
        sample_rate: u32,
        channels: u16,
        bits_per_sample: u16,
        frames_per_packet: u32,
    ) -> anyhow::Result<flume::Receiver<Vec<u8>>>;
    fn stop(&self);
    fn push_data(&self, _timestamp: u32, _data: Vec<u8>) {}
}
