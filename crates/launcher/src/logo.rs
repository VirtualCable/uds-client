use image::load_from_memory;
use eframe::egui::ColorImage;

const LOGO_BYTES: &[u8] = include_bytes!("../../../assets/img/uds-64.png");


pub fn load_logo() -> ColorImage {
    let img = load_from_memory(LOGO_BYTES).expect("imagen válida");
    let rgba = img.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    ColorImage::from_rgba_unmultiplied(size, &rgba)
}
