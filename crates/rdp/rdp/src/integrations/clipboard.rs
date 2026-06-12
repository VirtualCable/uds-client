// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.

use std::sync::Arc;

pub trait ClipboardIntegration: Send + Sync + std::fmt::Debug {
    fn start(&self, callback: Arc<dyn ClipboardCallback>);
    fn stop(&self);
    fn set_text(&self, text: &str) -> anyhow::Result<()>;
    fn get_text(&self) -> anyhow::Result<String>;
}

pub trait ClipboardCallback: Send + Sync {
    fn send_text_is_available(&self, text: &str);
}
