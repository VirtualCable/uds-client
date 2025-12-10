use std::sync::{Arc, RwLock};

use shared::log;

pub mod cliprdr;
pub mod disp;

#[derive(Clone, Debug)]
pub struct RdpChannels {
    disp: Option<disp::DispChannel>,
    cliprdr: Option<cliprdr::RdpClipboard>,

    // Helper for clipbrdr channel, to connect with native clipboard
    native: Option<Arc<RwLock<cliprdr::native::ClipboardNative>>>,
}

impl RdpChannels {
    pub fn new() -> Self {
        RdpChannels {
            disp: None,
            cliprdr: None,
            native: None,
        }
    }

    pub fn set_disp_ptr(&mut self, disp: *mut freerdp_sys::DispClientContext) {
        self.disp = Some(disp::DispChannel::new(disp));
    }

    pub fn clear_disp(&mut self) {
        self.disp = None;
    }

    pub fn disp(&self) -> Option<disp::DispChannel> {
        self.disp.clone()
    }

    pub fn set_cliprdr_ptr(&mut self, cliprdr: *mut freerdp_sys::CliprdrClientContext) {
        let clipboard = cliprdr::RdpClipboard::new(cliprdr);
        self.cliprdr = Some(clipboard.clone());
        self.native = cliprdr::native::ClipboardNative::new(clipboard);
    }

    pub fn clear_cliprdr(&mut self) {
        self.cliprdr = None;
    }

    pub fn cliprdr(&self) -> Option<cliprdr::RdpClipboard> {
        self.cliprdr.clone()
    }

    pub fn native(&self) -> Option<Arc<RwLock<cliprdr::native::ClipboardNative>>> {
        self.native.clone()
    }

    pub fn stop_native(&self) {
        if let Some(native) = &self.native {
            log::debug!("Stopping clipboard native watcher");
            native.write().unwrap().stop();
        }
    }
}

impl Default for RdpChannels {
    fn default() -> Self {
        Self::new()
    }
}
