use std::{env, path::PathBuf};

const FREERDP_ROOT_ENV_VAR: &str = "FREERDP_ROOT";
// On debian, for example, this path is /usr/include/freerdp3 and /usr/include/winpr3

fn main() {
    let (include_freerdp, include_winpr, lib_path) = if env::var(FREERDP_ROOT_ENV_VAR).is_ok() {
        let freerdp_path = env::var(FREERDP_ROOT_ENV_VAR).unwrap();
        (
            format!("{}/include/freerdp3", freerdp_path),
            format!("{}/include/winpr3", freerdp_path),
            format!("{}/lib", freerdp_path),
        )
    } else {
        // Try default paths
        (
            "/usr/include/freerdp3".to_string(),
            "/usr/include/winpr3".to_string(),
            "/usr/lib/x86_64-linux-gnu".to_string(),
        )
    };

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
