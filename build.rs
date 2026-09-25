//! Build script: stamp an ELF SONAME onto the Linux `cdylib`.
//!
//! The native SDK ships a versioned shared object (`libgibson.so.1`) whose
//! SONAME tracks the C ABI major version (`GIBSON_ABI_VERSION`, currently 1).
//! Baking `DT_SONAME = libgibson.so.1` makes a dynamic consumer record
//! `NEEDED = libgibson.so.1` (not the bare `libgibson.so`), so an ABI-incompatible
//! future major can coexist and consumers fail loudly rather than silently.
//!
//! `cargo:rustc-cdylib-link-arg` is the stable, cdylib-scoped mechanism — it does
//! not touch the `rlib` (crates.io Rust consumers), the `staticlib`, or the
//! example/integration-test binaries. `-Wl,-soname` is GNU ld / lld syntax, so it
//! is guarded to Linux; macOS (`-install_name`) and Windows have different or no
//! SONAME concepts and are out of scope for this Linux-x86_64 release.
//!
//! IMPORTANT: keep the SONAME major (`.so.1`) in lock-step with
//! `GIBSON_ABI_VERSION` in `include/gibson.h`. A real C ABI break bumps both.

fn main() {
    // Only the produced `.so` gains the SONAME; staging renames the build output
    // (`libgibson.so`) to `libgibson.so.1` and adds the `libgibson.so` dev symlink.
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libgibson.so.1");
}
