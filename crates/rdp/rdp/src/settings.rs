use zeroize::Zeroize;

#[allow(dead_code)]
#[derive(Zeroize, Debug, Clone)]
pub enum ScreenSize {
    Full,
    Fixed(u32, u32),
}

// TODO; fix fullscreen handling
impl ScreenSize {
    pub fn width(&self) -> u32 {
        match self {
            ScreenSize::Full => 0,
            ScreenSize::Fixed(w, _) => *w,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            ScreenSize::Full => 0,
            ScreenSize::Fixed(_, h) => *h,
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        matches!(self, ScreenSize::Full)
    }
}

#[derive(Zeroize, Debug)]
pub struct RdpSettings {
    pub server: String,
    pub port: u32,
    pub user: String,
    pub password: String,
    pub domain: String,
    pub verify_cert: bool,
    pub use_nla: bool,
    pub screen_size: ScreenSize,
    // Valid values for drives_to_redirect are "all" for all drives
    // % -> Home
    // * --> All drives
    // DynamicDrives --> Later connected drives
    pub drives_to_redirect: Vec<String>,
}

impl Default for RdpSettings {
    fn default() -> Self {
        RdpSettings {
            server: "".to_string(),
            port: 3389,
            user: "".to_string(),
            password: "".to_string(),
            domain: "".to_string(),
            verify_cert: false,
            screen_size: ScreenSize::Fixed(1024, 768),
            use_nla: false,
            drives_to_redirect: vec!["all".to_string()], // By default, redirect all drives.
        }
    }
}
