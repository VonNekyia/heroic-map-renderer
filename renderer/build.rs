//! Unter Windows bekommt das Binär ein Manifest mit dem Segment-Heap.
//!
//! Der Rust-Teil allokiert über mimalloc, libwebp aber über `malloc` der
//! C-Laufzeit, ohne Haken für einen eigenen Allokator. Je Kachel sind das
//! rund 2 MB, die der gewöhnliche Windows-Heap beim Freigeben ans System
//! zurückgibt und bei der nächsten Kachel neu holt: gut 500 Seitenfehler je
//! Kachel, die sich auf vielen Threads stauen. Der Segment-Heap behält sie.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("segmentheap.manifest");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        manifest.display()
    );
}
