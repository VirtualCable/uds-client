use zeroize::Zeroize;

use super::geom::ScreenSize;

#[derive(Zeroize, Debug)]
pub struct RdpSettings {
    #[zeroize(skip)]
    pub server: String,
    #[zeroize(skip)]
    pub port: u32,
    pub user: String,
    pub password: String,
    pub domain: String,
    #[zeroize(skip)]
    pub verify_cert: bool,
    #[zeroize(skip)]
    pub use_nla: bool,
    #[zeroize(skip)]
    pub screen_size: ScreenSize,
    // Valid values for drives_to_redirect are "all" for all drives
    // % -> Home
    // * --> All drives
    // DynamicDrives --> Later connected drives
    #[zeroize(skip)]
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
