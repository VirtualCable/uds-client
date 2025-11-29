#[derive(Debug, Copy, Clone)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ScreenSize {
    Full,
    Fixed(u32, u32),
}

// TODO; fix fullscreen handling
impl ScreenSize {
    pub fn width(&self) -> u32 {
        match self {
            ScreenSize::Full => 1920,
            ScreenSize::Fixed(w, _) => *w,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            ScreenSize::Full => 1080,
            ScreenSize::Fixed(_, h) => *h,
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        matches!(self, ScreenSize::Full)
    }
}
