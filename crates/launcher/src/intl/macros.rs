// BSD 3-Clause License
// Copyright (c) 2026, Virtual Cable S.L.
// All rights reserved.
// Authors: Adolfo Gómez, dkmaster at dkmon dot com

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_returns_raw() {
        assert_eq!(interpolate("hello", &[]), "hello");
    }

    #[test]
    fn empty_raw() {
        assert_eq!(interpolate("", &[&"x"]), "");
    }

    #[test]
    fn single_replacement() {
        assert_eq!(interpolate("hello {}", &[&"world"]), "hello world");
    }

    #[test]
    fn multiple_replacements() {
        assert_eq!(interpolate("{} + {} = {}", &[&1, &2, &3]), "1 + 2 = 3");
    }

    #[test]
    fn fewer_placeholders_than_args() {
        assert_eq!(interpolate("{}", &[&1, &2, &3]), "1");
    }

    #[test]
    fn more_placeholders_than_args() {
        assert_eq!(interpolate("{} {} {}", &[&1]), "1 {} {}");
    }

    #[test]
    fn no_placeholders() {
        assert_eq!(interpolate("no markers here", &[&1, &2]), "no markers here");
    }

    #[test]
    fn special_chars_in_args() {
        assert_eq!(interpolate("val={}", &[&"foo{}bar"]), "val=foo{}bar");
    }

    #[test]
    fn display_types() {
        assert_eq!(interpolate("{}", &[&42i32]), "42");
        assert_eq!(interpolate("{}", &[&3.22f64]), "3.22");
        assert_eq!(interpolate("{}", &[&true]), "true");
    }
}
