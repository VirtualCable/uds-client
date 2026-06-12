use super::VideoEncoder;

pub struct Yuy2Encoder {
    width: u32,
    height: u32,
}

impl Yuy2Encoder {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
        }
    }
}

impl Default for Yuy2Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoEncoder for Yuy2Encoder {
    fn init(&mut self, width: u32, height: u32, _fps: u32, _quality: u32) -> Result<(), String> {
        self.width = width;
        self.height = height;
        Ok(())
    }

    fn encode(&mut self, rgb: &[u8]) -> Result<Vec<u8>, String> {
        let width = self.width;
        let height = self.height;
        let num_pixels = (width * height) as usize;
        let mut yuy2 = vec![0u8; num_pixels * 2];

        for i in (0..num_pixels).step_by(2) {
            let idx1 = i * 3;
            let idx2 = (i + 1) * 3;

            if idx2 + 2 >= rgb.len() {
                break;
            }

            let r1 = rgb[idx1] as f32;
            let g1 = rgb[idx1 + 1] as f32;
            let b1 = rgb[idx1 + 2] as f32;

            let r2 = rgb[idx2] as f32;
            let g2 = rgb[idx2 + 1] as f32;
            let b2 = rgb[idx2 + 2] as f32;

            // Luminance Y1 and Y2
            let y1 = (0.299 * r1 + 0.587 * g1 + 0.114 * b1) as u8;
            let y2 = (0.299 * r2 + 0.587 * g2 + 0.114 * b2) as u8;

            // Chroma U and V (averaged)
            let r_avg = (r1 + r2) / 2.0;
            let g_avg = (g1 + g2) / 2.0;
            let b_avg = (b1 + b2) / 2.0;

            let u = (-0.168736 * r_avg - 0.331264 * g_avg + 0.5 * b_avg + 128.0) as u8;
            let v = (0.5 * r_avg - 0.418688 * g_avg - 0.081312 * b_avg + 128.0) as u8;

            let dest_idx = i * 2;
            yuy2[dest_idx] = y1;
            yuy2[dest_idx + 1] = u;
            yuy2[dest_idx + 2] = y2;
            yuy2[dest_idx + 3] = v;
        }

        Ok(yuy2)
    }
}
