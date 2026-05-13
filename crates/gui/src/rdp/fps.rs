use std::sync::atomic::{AtomicBool, Ordering};

#[allow(dead_code)]
pub struct Fps {
    pub last_instant: std::time::Instant,
    frames: Vec<std::time::Instant>,
    pub enabled: AtomicBool,
}

impl Fps {
    pub fn new() -> Self {
        Self {
            last_instant: std::time::Instant::now(),
            frames: Vec::new(),
            enabled: AtomicBool::new(false),
        }
    }
    pub fn record(&mut self) {
        let now = std::time::Instant::now();
        self.frames
            .retain(|t| now.duration_since(*t).as_secs_f32() < 2.0);
        self.frames.push(now);
    }
    pub fn toggle(&self) {
        let v = self.enabled.load(Ordering::Relaxed);
        self.enabled.store(!v, Ordering::Relaxed);
    }
    pub fn average(&self) -> f32 {
        let now = std::time::Instant::now();
        let recent: Vec<_> = self
            .frames
            .iter()
            .filter(|t| now.duration_since(**t).as_secs_f32() < 1.0)
            .collect();
        recent.len() as f32
    }
}
