// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

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
        // Fallback to English
        let fallback = "en/LC_MESSAGES/myapp.mo";
        let file = Locales::get(fallback).expect("no fallback catalog found");
        Catalog::parse(file.data.as_ref()).expect("failed to parse fallback catalog")
    }
});

pub fn get_catalog() -> &'static Catalog {
    &CATALOG
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zh_simplified_variants() {
        assert_eq!(normalize_lang("zh"), "zh_CN");
        assert_eq!(normalize_lang("zh-CN"), "zh_CN");
        assert_eq!(normalize_lang("zh_Hans"), "zh_CN");
    }

    #[test]
    fn zh_traditional_variants() {
        assert_eq!(normalize_lang("zh-TW"), "zh_TW");
        assert_eq!(normalize_lang("zh_Hant"), "zh_TW");
    }

    #[test]
    fn other_languages_passthrough() {
        assert_eq!(normalize_lang("en"), "en");
        assert_eq!(normalize_lang("es"), "es");
        assert_eq!(normalize_lang("fr"), "fr");
        assert_eq!(normalize_lang("de"), "de");
        assert_eq!(normalize_lang("ja"), "ja");
        assert_eq!(normalize_lang("pt-BR"), "pt-BR");
    }

    #[test]
    fn empty_passthrough() {
        assert_eq!(normalize_lang(""), "");
    }
}
