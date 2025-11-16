use gettext::Catalog;
use rust_embed::RustEmbed;
use std::sync::LazyLock;
use sys_locale::get_locale;

#[macro_use]
pub mod macros;

#[derive(RustEmbed)]
#[folder = "locales"]
struct Locales;

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    // System locale
    let sys_lang: String = get_locale().unwrap_or_else(|| "en".to_string());
    let lang = sys_lang.split('_').next().unwrap_or("en"); // Just en, es, it, etc.

    // Construir la ruta dentro del embed
    let path = format!("{}/LC_MESSAGES/myapp.mo", lang);

    // Intentar cargar el catálogo
    if let Some(file) = Locales::get(&path) {
        Catalog::parse(file.data.as_ref()).expect("failed to parse catalog")
    } else {
        // Fallback a inglés si no existe
        let fallback = "en/LC_MESSAGES/myapp.mo";
        let file = Locales::get(fallback).expect("no fallback catalog found");
        Catalog::parse(file.data.as_ref()).expect("failed to parse fallback catalog")
    }
});

#[cfg(test)]
mod tests {
    use super::CATALOG;

    #[test]
    fn test_translation() {
        let translated = tr!("Hello, world!");
        assert_eq!(translated, "¡Hola, mundo!"); // Assuming Spanish translation
    }

    #[test]
    fn test_plural_translation() {
        let n = 3;
        let translated = CATALOG
            .ngettext("One file", "{} files", n)
            .replace("{}", &n.to_string());
        assert_eq!(translated, "3 archivos"); // Assuming Spanish translation
    }
}
