#[macro_export]
macro_rules! tr {
    ($msg:expr) => {
        $crate::intl::CATALOG.gettext($msg)
    };
    ($sing:expr, $plur:expr, $n:expr) => {
        $crate::intl::CATALOG.ngettext($sing, $plur, $n)
    };
}