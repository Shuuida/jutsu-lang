use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    // If the feature is NOT enabled, we stop execution and do not perform static linking
    #[cfg(not(feature = "static_ffi"))]
    return;

    // If the feature IS enabled, we configure the C++ compiler
    #[cfg(feature = "static_ffi")]
    {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Failed to get CARGO_MANIFEST_DIR");
        let lib_path = PathBuf::from(manifest_dir);

        println!("cargo:rustc-link-search=native={}", lib_path.display());
        println!("cargo:rustc-link-lib=dylib=llama");
    }
}