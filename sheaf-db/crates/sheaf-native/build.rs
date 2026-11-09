// build.rs - composition root for the three native tier builds.
//
// Each tier probes its own toolchain and falls back independently:
//   build/kernel.rs  - C++23 math kernel  (zig c++ / clang++ / g++)
//   build/storage.rs - Zig BCSF storage   (zig only)
//   build/wal.rs     - C11 WAL            (zig cc / clang / gcc)
//
// A missing toolchain never fails the build. The matching Rust fallback
// adapter stays active, so the workspace builds and tests everywhere.
// Feature force-fallback or env SHEAF_NATIVE_DISABLE=1 skips all tiers.

#[path = "build/toolchain.rs"]
mod toolchain;
#[path = "build/kernel.rs"]
mod kernel;
#[path = "build/storage.rs"]
mod storage;
#[path = "build/wal.rs"]
mod wal;

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // crates/sheaf-native -> workspace root -> native/
    let native_root = manifest
        .join("../../native")
        .canonicalize()
        .unwrap_or_else(|_| manifest.join("../../native"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    println!("cargo:rerun-if-changed={}", native_root.display());
    println!("cargo:rerun-if-env-changed=SHEAF_NATIVE_DISABLE");
    // Declare every custom cfg so rustc does not warn about unexpected cfgs.
    println!("cargo:rustc-check-cfg=cfg(sheaf_native_kernel)");
    println!("cargo:rustc-check-cfg=cfg(sheaf_native_storage)");
    println!("cargo:rustc-check-cfg=cfg(sheaf_native_wal)");

    if env::var("CARGO_FEATURE_FORCE_FALLBACK").is_ok()
        || env::var("SHEAF_NATIVE_DISABLE").map(|v| v == "1").unwrap_or(false)
    {
        println!("cargo:warning=sheaf-native: fallback forced, native tiers not compiled");
        return;
    }

    kernel::build(&native_root, &out_dir);
    storage::build(&native_root, &out_dir);
    wal::build(&native_root, &out_dir);
}