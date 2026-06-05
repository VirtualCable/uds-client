pub trait VideoEncoder: Send {
    fn init(&mut self, width: u32, height: u32, fps: u32) -> Result<(), String>;
    fn encode(&mut self, rgb: &[u8]) -> Result<Vec<u8>, String>;
}

pub struct RawEncoder;

impl VideoEncoder for RawEncoder {
    fn init(&mut self, _width: u32, _height: u32, _fps: u32) -> Result<(), String> {
        Ok(())
    }

    fn encode(&mut self, rgb: &[u8]) -> Result<Vec<u8>, String> {
        Ok(rgb.to_vec())
    }
}

mod mjpeg;
mod yuy2;

pub use mjpeg::MjpegEncoder;
pub use yuy2::Yuy2Encoder;
