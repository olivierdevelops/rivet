//! Linker settings for the cdylib (ADR-0005 decision 6, RES-2026-0004 E2):
//! macOS records `@rpath/librivet.dylib` as the install name (not an absolute
//! path under target/), Linux sets the soname `librivet.so`.
fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match os.as_str() {
        "macos" | "ios" => {
            println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/librivet.dylib")
        }
        "linux" | "android" | "freebsd" => {
            println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,librivet.so")
        }
        _ => {}
    }
    println!("cargo:rerun-if-changed=build.rs");
}
