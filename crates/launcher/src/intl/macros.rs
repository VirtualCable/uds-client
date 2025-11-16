pub fn interpolate(raw: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut result = raw.to_string();
    for arg in args {
        if result.contains("{}") {
            result = result.replacen("{}", &arg.to_string(), 1);
        }
    }
    result
}

#[macro_export]
macro_rules! tr {
    // Simple translation
    ($msg:expr) => {
        $crate::intl::CATALOG.gettext($msg)
    };

    // Translation with parameters
    ($msg:expr, $($arg:expr),+ $(,)?) => {{
        let raw = $crate::intl::CATALOG.gettext($msg);
        $crate::intl::macros::interpolate(&raw, &[ $( &$arg ),+ ])
    }};

    // Plural translation
    ($sing:expr, $plur:expr, $n:expr) => {{
        let raw = $crate::intl::CATALOG.ngettext($sing, $plur, $n);
        $crate::intl::macros::interpolate(&raw, &[ &$n ])
    }};
}
