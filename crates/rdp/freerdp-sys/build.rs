use std::{env, path::PathBuf};

fn main() {
    // 🔹 Name of the environment variable we'll use
    const ENV_VAR: &str = "FREERDP_PATH";
    // 🔹 Default value if it's not defined
    #[cfg(windows)]
    const DEFAULT_PATH: &str = "Z:/dev/freerdp";
    #[cfg(target_os = "linux")]
    const DEFAULT_PATH: &str = "/home/dkmaster/projects/uds/5.0/repos/rdp/local";
    #[cfg(target_os = "macos")]
    const DEFAULT_PATH: &str = "/Users/dkmaster/projects/rdp/local";

    // Read the environment variable or use the default value
    let freerdp_path = env::var(ENV_VAR).unwrap_or_else(|_| DEFAULT_PATH.to_string());

    let include_freerdp = format!("{}/include/freerdp3", freerdp_path);
    let include_winpr = format!("{}/include/winpr3", freerdp_path);
    let lib_path = format!("{}/lib", freerdp_path);

    // Build the C shim
    cc::Build::new()
        .file("src/shims/get_access_token_wrapper.c")
        .include(include_freerdp.clone())
        .include(include_winpr.clone())
        .compile("freerdp_shims");

    // Link to the required libraries
    println!("cargo:rustc-link-search=native={}", lib_path);
    println!("cargo:rustc-link-lib=freerdp3");
    println!("cargo:rustc-link-lib=winpr3");
    println!("cargo:rustc-link-lib=freerdp-client3");
    // Add more here if we need them

    // Generate bindings with bindgen
    // If wrapper.h changes, rerun this build script
    println!("cargo:rerun-if-changed=wrapper.h");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_freerdp))
        .clang_arg(format!("-I{}", include_winpr))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .opaque_type("NT_CONSOLE_PROPS")
        .opaque_type("NT_FE_CONSOLE_PROPS")
        .opaque_type("EXP_DARWIN_LINK")
        .opaque_type("CABINETSTATE")
        .opaque_type("SHELLSTATEA")
        .opaque_type("SHELLSTATEW")
        .opaque_type("SHELLFLAGSTATE")
        .generate()
        .expect("Bindings could not be generated");

    // Save to the $OUT_DIR/bindings.rs file
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings.write_to_file(&out_path).unwrap();
}
