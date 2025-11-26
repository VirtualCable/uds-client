use winit::keyboard::KeyCode;

mod scancodes;
mod winit_keys;

pub use scancodes::RdpScanCode;

impl RdpScanCode {
    pub fn get_from_key(key: Option<&KeyCode>) -> Option<Self> {
        if let Some(k) = key {
            RdpScanCode::from_egui_key(k)
        } else {
            None
        }
    }
}
