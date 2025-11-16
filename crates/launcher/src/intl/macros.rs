#[macro_export]
macro_rules! tr {
    // Singular translation
    ($msg:expr) => {
        $crate::intl::CATALOG.gettext($msg)
    };

    // Plural translation with automatic interpolation
    ($sing:expr, $plur:expr, $n:expr) => {{
        let raw = $crate::intl::CATALOG.ngettext($sing, $plur, $n);
        raw.replace("{}", &$n.to_string())
    }};
}