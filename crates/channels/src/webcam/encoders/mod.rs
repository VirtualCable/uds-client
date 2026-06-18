use anyhow::Result;

pub trait VideoEncoder: Send {
    fn init(&mut self, width: u32, height: u32, fps: u32, quality: u32) -> Result<()>;
    fn encode(&mut self, rgb: &[u8]) -> Result<Vec<u8>>;
}

pub struct RawEncoder;

impl VideoEncoder for RawEncoder {
    fn init(&mut self, _width: u32, _height: u32, _fps: u32, _quality: u32) -> Result<()> {
        Ok(())
    }

    fn encode(&mut self, rgb: &[u8]) -> Result<Vec<u8>> {
        Ok(rgb.to_vec())
    }
}

mod h264;
mod mjpeg;
mod yuy2;

pub use h264::H264Encoder;
pub use mjpeg::MjpegEncoder;
pub use yuy2::Yuy2Encoder;
