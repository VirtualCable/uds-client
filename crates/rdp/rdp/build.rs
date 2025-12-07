#![allow(dead_code)]
use std::{env, fs, io, path::PathBuf};

fn copy_if_different(src: &PathBuf, dst: &PathBuf) -> io::Result<()> {
    if !dst.exists() || fs::metadata(src)?.len() != fs::metadata(dst)?.len() {
        // Si no existe o el tamaño es distinto, copiamos
        println!("cargo:warning=Copying {:?}", src.file_name().unwrap());
        fs::copy(src, dst)?;
    }
    Ok(())
}

fn copy_windows_dlls() {
    // Out dir is our parent directory + "local_dlls"
    let out_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("local_dlls");

    fs::create_dir_all(&out_dir).unwrap();

    let freerdp_bin = PathBuf::from(r"Z:\dev\freerdp\bin");
    let vcpkg = PathBuf::from(r"Z:\dev\vcpkg");
    let vcpkg_bin = PathBuf::from(format!("{}/installed/x64-windows/bin", vcpkg.display()));
    let vcpkg_lib = PathBuf::from(format!("{}/installed/x64-windows/lib", vcpkg.display()));

    println!("cargo:rerun-if-changed={}", out_dir.display());

    // FreeRDP DLLs
    let freerdp_dlls = [
        "freerdp3.dll",
        "freerdp-client3.dll",
        "winpr3.dll",
        "winpr-tools3.dll",
    ];

    // vcpkg DLLs
    let vcpkg_dlls = [
        "zlib1.dll",
        // "bz2.dll",
        "libssl-3-x64.dll",
        "libcrypto-3-x64.dll",
        "libusb-1.0.dll",
        // "libpng16.dll",
        // "vorbisfile.dll",
        // "vorbis.dll",
        // "ogg.dll",
        // "brotlidec.dll",
        // "brotlicommon.dll",
        "libmp3lame.dll",
        "avcodec-61.dll",
        "avutil-59.dll",
        "swscale-8.dll",
        "swresample-5.dll",
        "openh264-7.dll",
        "avcodec-61.dll",
        "swscale-8.dll",
        "avutil-59.dll",
        "swresample-5.dll",
        "cjson.dll",
    ];

    for dll in freerdp_dlls {
        copy_if_different(&freerdp_bin.join(dll), &out_dir.join(dll)).unwrap();
        println!("cargo:rerun-if-changed={}", freerdp_bin.join(dll).display());
    }

    for dll in vcpkg_dlls {
        copy_if_different(&vcpkg_bin.join(dll), &out_dir.join(dll)).unwrap();
        println!("cargo:rerun-if-changed={}", vcpkg_bin.join(dll).display());
    }

    println!("cargo:rustc-link-search=native={}", vcpkg_lib.display());
}

fn linux_build() {
    // Placeholder for Linux-specific build steps
    // Set include paths (.../freerdp3 and ..../winpr3) for .h files if needed
}

fn main() {
    #[cfg(windows)]
    copy_windows_dlls();
    #[cfg(target_os = "linux")]
    linux_build();
}
