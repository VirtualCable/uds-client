pub struct Cursor {
    pub data: Vec<u8>,
    pub hot_x: u32,
    pub hot_y: u32,
    pub w: u32,
    pub h: u32,
    pub visible: bool,
    pub x: f32,
    pub y: f32,
    pub scale: f64,
}

impl Cursor {
    pub fn new(cursor_scale: f64) -> Self {
        Self {
            data: Vec::new(),
            hot_x: 0,
            hot_y: 0,
            w: 0,
            h: 0,
            visible: false,
            x: 0.0,
            y: 0.0,
            scale: cursor_scale,
        }
    }

    pub fn set_icon(&mut self, data: Vec<u8>, x: u32, y: u32, width: u32, height: u32) {
        self.data = data;
        self.hot_x = x;
        self.hot_y = y;
        self.w = width;
        self.h = height;
        self.visible = width > 0 && height > 0;
    }

    pub fn build_overlay(&self) -> Option<crate::wgpu_render::OverlayParams<'_>> {
        if !self.visible || self.data.is_empty() {
            return None;
        }
        let (hot_x, hot_y) =
            crate::monitor::logic_2_phys_pos((self.hot_x as i32, self.hot_y as i32), self.scale);
        Some(crate::wgpu_render::OverlayParams {
            rgba: self.data.as_slice(),
            width: self.w,
            height: self.h,
            x: self.x - hot_x as f32,
            y: self.y - hot_y as f32,
            scale: self.scale as f32,
        })
    }
}
