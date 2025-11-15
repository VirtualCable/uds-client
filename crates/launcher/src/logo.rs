use image::load_from_memory;
use eframe::egui::{ColorImage, IconData};

const LOGO_BYTES: &[u8] = include_bytes!("../../../assets/img/uds-64.png");
const ICON_BYTES: &[u8] = include_bytes!("../../../assets/img/uds-icon.png");

pub fn load_logo() -> ColorImage {
    let img = load_from_memory(LOGO_BYTES).expect("Failed to load image");
    let rgba = img.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    ColorImage::from_rgba_unmultiplied(size, &rgba)
}

pub fn load_icon() -> IconData {
    let img = load_from_memory(ICON_BYTES).expect("Failed to load icon");
    let rgba = img.to_rgba8();
    IconData {
        width: img.width() as u32,
        height: img.height() as u32,
        rgba: rgba.into_raw(),
    }
}