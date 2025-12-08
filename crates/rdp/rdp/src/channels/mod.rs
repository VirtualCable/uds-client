use crate::utils;

pub mod disp;
pub mod cliprdr;

#[derive(Clone, Debug)]
pub struct RdpChannels {
    disp: Option<utils::SafePtr<freerdp_sys::DispClientContext>>,
    cliprdr: Option<utils::SafePtr<freerdp_sys::CliprdrClientContext>>,
}

impl RdpChannels {
    pub fn new() -> Self {
        RdpChannels {
            disp: None,
            cliprdr: None,
        }
    }

    pub fn set_disp(&mut self, disp: *mut freerdp_sys::DispClientContext) {
        self.disp = utils::SafePtr::new(disp);
    }

    pub fn clear_disp(&mut self) {
        self.disp = None;
    }

    pub fn disp(&self) -> Option<utils::SafePtr<freerdp_sys::DispClientContext>> {
        self.disp
    }

    pub fn set_cliprdr(&mut self, cliprdr: *mut freerdp_sys::CliprdrClientContext) {
        self.cliprdr = utils::SafePtr::new(cliprdr);
    }

    pub fn clear_cliprdr(&mut self) {
        self.cliprdr = None;
    }

    pub fn cliprdr(&self) -> Option<utils::SafePtr<freerdp_sys::CliprdrClientContext>> {
        self.cliprdr
    }
}

impl Default for RdpChannels {
    fn default() -> Self {
        Self::new()
    }
}
