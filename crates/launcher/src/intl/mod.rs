use gettext::Catalog;
use rust_embed::RustEmbed;
use std::sync::LazyLock;
use sys_locale::get_locale;

#[macro_use]
pub mod macros;

#[derive(RustEmbed)]
#[folder = "locales"]
struct Locales;

fn normalize_lang(lang: &str) -> &str {
    match lang {
        "zh" | "zh-CN" | "zh_Hans" => "zh_CN", // Simplified
        "zh-TW" | "zh_Hant" => "zh_TW",        // Traditional
        _ => lang,
    }
}

pub static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    let sys_lang = get_locale().unwrap_or_else(|| "en".to_string());
    let lang = sys_lang.split(&['-', '_'][..]).next().unwrap_or("en");
    let norm = normalize_lang(lang);

    let path = format!("{}/LC_MESSAGES/udslauncher.mo", norm);

    if let Some(file) = Locales::get(&path) {
        Catalog::parse(file.data.as_ref()).expect("failed to parse catalog")
    } else {
        // Fallback a inglés
        let fallback = "en/LC_MESSAGES/myapp.mo";
        let file = Locales::get(fallback).expect("no fallback catalog found");
        Catalog::parse(file.data.as_ref()).expect("failed to parse fallback catalog")
    }
});

pub fn get_catalog() -> &'static Catalog {
    &CATALOG
}
