//! Unter Windows bekommt das Binär ein Manifest mit dem Segment-Heap.
//!
//! Der Rust-Teil allokiert über mimalloc, libwebp aber über `malloc` der
//! C-Laufzeit, ohne Haken für einen eigenen Allokator. Der gewöhnliche
//! Windows-Heap gibt diesen Speicher nach jeder Kachel ans System zurück,
//! der Segment-Heap behält ihn.
//! Siehe docs/entscheidungen/0029-segment-heap-fuer-libwebp.md.

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
