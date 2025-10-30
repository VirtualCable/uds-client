// uds-build/src/lib.rs
use chrono::{Datelike, Utc, NaiveDate};
use winres::WindowsResource;

pub struct BuildInfo<'a> {
    pub product_name: &'a str,
    pub description: &'a str,
    pub icon: Option<&'a str>,
    pub bmp: Option<&'a str>,
}

pub fn build_windows(build_info: BuildInfo) {
    let icon = build_info.icon.unwrap_or("../../assets/img/uds.ico");
    let bmp = build_info.bmp.unwrap_or("../../assets/img/uds.bmp");
    println!("cargo:rerun-if-changed={icon}");
    println!("cargo:rerun-if-changed={bmp}");

    let current_year = Utc::now().year();
    let base_date = NaiveDate::from_ymd_opt(1972, 7, 1).unwrap();
    let today = Utc::now().date_naive();
    let build_days = (today - base_date).num_days();

    let (major, minor, patch, build) = (
        env!("CARGO_PKG_VERSION_MAJOR").parse::<u64>().unwrap(),
        env!("CARGO_PKG_VERSION_MINOR").parse::<u64>().unwrap(),
        env!("CARGO_PKG_VERSION_PATCH").parse::<u64>().unwrap(),
        build_days as u64,
    );

    let version: u64 = (major << 48) | (minor << 32) | (patch << 16) | build;

    let mut res = WindowsResource::new();
    res.set_icon(icon);

    res.set_version_info(winres::VersionInfo::FILEVERSION, version);
    res.set_version_info(winres::VersionInfo::PRODUCTVERSION, version);

    res.set_language(0x0409);

    res.set("FileVersion", &format!("{major}.{minor}.{patch}.{build}"));
    res.set("ProductVersion", &format!("{major}.{minor}.{patch}.{build}"));
    res.set("ProductName", build_info.product_name);
    res.set("FileDescription", build_info.description);
    res.set(
        "LegalCopyright",
        format!("Copyright © 2012-{current_year} Virtual Cable S.L.U.").as_str(),
    );
    res.set("CompanyName", "Virtual Cable S.L.U.");

    res.append_rc_content(&format!(r##"101 BITMAP DISCARDABLE "{}""##, bmp));

    res.compile().unwrap();
}
