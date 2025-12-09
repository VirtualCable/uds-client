pub mod disp;
pub mod cliprdr;

#[derive(Clone, Debug)]
pub struct RdpChannels {
    disp: Option<disp::DispChannel>,
    cliprdr: Option<cliprdr::Clipboard>,
}

impl RdpChannels {
    pub fn new() -> Self {
        RdpChannels {
            disp: None,
            cliprdr: None,
        }
    }

    pub fn set_disp(&mut self, disp: *mut freerdp_sys::DispClientContext) {
        self.disp = Some(disp::DispChannel::new(disp));
    }

    pub fn clear_disp(&mut self) {
        self.disp = None;
    }

    pub fn disp(&self) -> Option<disp::DispChannel> {
        self.disp.clone()
    }

    pub fn set_cliprdr(&mut self, cliprdr: *mut freerdp_sys::CliprdrClientContext) {
        self.cliprdr = Some(cliprdr::Clipboard::new(cliprdr));
    }

    pub fn clear_cliprdr(&mut self) {
        self.cliprdr = None;
    }

    pub fn cliprdr(&self) -> Option<cliprdr::Clipboard> {
        self.cliprdr.clone()
    }
}

impl Default for RdpChannels {
    fn default() -> Self {
        Self::new()
    }
}
