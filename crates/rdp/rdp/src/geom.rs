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

/// Methods for ScreenSize
/// values returned for Full are default valid sizes for windowed mode
/// after exiting fullscreen, as we don't have access to the actual
/// screen size here for fullscreen
/// Currently, we use a proportional size of 16:9 for fullscreen default
impl ScreenSize {
    pub fn width(&self) -> u32 {
        match self {
            ScreenSize::Full => 1200,
            ScreenSize::Fixed(w, _) => *w,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            ScreenSize::Full => 675,
            ScreenSize::Fixed(_, h) => *h,
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        matches!(self, ScreenSize::Full)
    }

    pub fn get_fixed_size(&self) -> Option<(u32, u32)> {
        match self {
            ScreenSize::Fixed(w, h) => Some((*w, *h)),
            ScreenSize::Full => None,
        }
    }
}
