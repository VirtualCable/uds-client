// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.
// All rights reserved.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
use image::load_from_memory;

const LOGO_BYTES: &[u8] = include_bytes!("../../../assets/img/uds-64.png");
const ICON_BYTES: &[u8] = include_bytes!("../../../assets/img/uds-icon.png");

pub struct LogoImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn load_logo() -> LogoImage {
    let img = load_from_memory(LOGO_BYTES).expect("Failed to load image");
    let rgba = img.to_rgba8();
    LogoImage {
        width: img.width(),
        height: img.height(),
        rgba: rgba.into_raw(),
    }
}

pub fn load_icon() -> winit::window::Icon {
    let img = load_from_memory(ICON_BYTES).expect("Failed to load icon");
    let rgba = img.to_rgba8();
    winit::window::Icon::from_rgba(rgba.into_raw(), img.width(), img.height())
        .expect("Failed to create icon")
}
