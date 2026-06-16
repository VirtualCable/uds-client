pub struct StreamState {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub format: u32,
    pub color_offset: u8,
}

pub fn generate_mock_frame(s: &mut StreamState) -> Vec<u8> {
    s.color_offset = s.color_offset.wrapping_add(4);
    let bar_pos = (s.color_offset as u32 * 2) % s.width;
    let mut rgb = vec![0u8; (s.width * s.height * 3) as usize];

    for y in 0..s.height {
        for x in 0..s.width {
            let idx = ((y * s.width + x) * 3) as usize;
            if x >= bar_pos && x < bar_pos + 40 {
                // Moving red bar
                rgb[idx] = 255;
                rgb[idx + 1] = 50;
                rgb[idx + 2] = 50;
            } else {
                // Moving blue/green gradient background
                rgb[idx] = 20;
                rgb[idx + 1] = (y % 256) as u8;
                rgb[idx + 2] = s.color_offset;
            }
        }
    }
    rgb
}
