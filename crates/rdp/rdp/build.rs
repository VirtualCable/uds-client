// BSD 3-Clause License
// Copyright (c) 2025, Virtual Cable S.L.U.
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice,
//    this list of conditions and the following disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice,
//    this list of conditions and the following disclaimer in the documentation
//    and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors
//    may be used to endorse or promote products derived from this software
//    without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

// Authors: Adolfo Gómez, dkmaster at dkmon dot com
#![allow(dead_code)]
use std::{env, fs, io, path::PathBuf};

fn copy_if_different(src: &PathBuf, dst: &PathBuf) -> io::Result<()> {
    if !dst.exists() || fs::metadata(src)?.len() != fs::metadata(dst)?.len() {
        // If it doesn't exist or its size differs, copy it
        println!("cargo:warning=Copying {:?}", src.file_name().unwrap());
        fs::copy(src, dst)?;
    }
    Ok(())
}

fn copy_windows_dlls() {
    const FREERDP_ROOT_ENV_VAR: &str = "FREERDP_ROOT";
    const VCPKG_ROOT_ENV_VAR: &str = "VCPKG_ROOT";

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
    let freerdp_path = env::var(FREERDP_ROOT_ENV_VAR).unwrap_or_else(|_| {
        panic!(
            "Environment variable {} is not set. Please set it to the FreeRDP installation path.",
            FREERDP_ROOT_ENV_VAR
        );
    });
    let vcpkg_root = env::var(VCPKG_ROOT_ENV_VAR).unwrap_or_else(|_| {
        panic!(
            "Environment variable {} is not set. Please set it to the vcpkg installation path.",
            VCPKG_ROOT_ENV_VAR
        );
    });

    let freerdp_bin = PathBuf::from(format!("{}/bin", freerdp_path));
    let vcpkg_bin = PathBuf::from(format!("{}/installed/x64-windows/bin", vcpkg_root));
    let vcpkg_lib = PathBuf::from(format!("{}/installed/x64-windows/lib", vcpkg_root));

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

        // Related to ffmpeg, for video decoding
        // "libmp3lame.dll",
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

fn main() {
    #[cfg(windows)]
    copy_windows_dlls();
    // Currently, no additional steps are needed for other platforms
}
