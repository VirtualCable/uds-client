use super::VideoEncoder;
use turbojpeg::{Image, OutputBuf, PixelFormat};
use shared::log;

pub struct MjpegEncoder {
    width: u32,
    height: u32,
    compressor: Option<turbojpeg::Compressor>,
}

impl MjpegEncoder {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            compressor: None,
        }
    }
}

impl Default for MjpegEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoEncoder for MjpegEncoder {
    fn init(&mut self, width: u32, height: u32, _fps: u32) -> Result<(), String> {
        self.width = width;
        self.height = height;
        if self.compressor.is_none() {
            match turbojpeg::Compressor::new() {
                Ok(c) => self.compressor = Some(c),
                Err(e) => return Err(format!("Failed to create turbojpeg compressor: {e}")),
            }
        }
        Ok(())
    }

    fn encode(&mut self, rgb: &[u8]) -> Result<Vec<u8>, String> {
        if let Some(ref mut compressor) = self.compressor {
            let image = Image {
                pixels: rgb,
                width: self.width as usize,
                height: self.height as usize,
                pitch: (self.width * 3) as usize,
                format: PixelFormat::RGB,
            };
            let mut output = OutputBuf::new_owned();
            match compressor.compress(image, &mut output) {
                Ok(_) => {
                    let jpeg = output.to_vec();
                    if !jpeg.is_empty() {
                        return Ok(jpeg);
                    }
                }
                Err(e) => {
                    log::error!("MJPEG compression failed: {e}");
                }
            }
        }
        Ok(rgb.to_vec()) // Fallback
    }
}
