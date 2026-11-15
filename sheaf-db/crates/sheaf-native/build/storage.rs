// storage.rs - build the Zig physical storage tier into a static library.
//
// `zig build-lib` compiles native/storage-zig/src/exports.zig as the root
// module. The root imports the spine, arena, and BCSF modules, so one
// command builds the whole tier without depending on build.zig install
// steps. The archive name follows the target linker convention.
//
// Only zig can build this tier. Without zig the Rust storage fallback
// stays active and the workspace keeps building.

use std::path::Path;
use std::process::Command;

use super::toolchain as tc;

pub fn build(native_root: &Path, out_dir: &Path) {
    if !tc::probe("zig", "version") {
        println!(
            "cargo:warning=sheaf-native: zig not found. The pure-Rust storage \
             fallback stays active. Install zig 0.14.x and rebuild to activate \
             the Zig BCSF storage tier."
        );
        return;
    }

    let root = native_root.join("storage-zig/src/exports.zig");
    let lib = out_dir.join(tc::archive_name("sheaf_storage"));

    let mut cmd = Command::new("zig");
    cmd.arg("build-lib")
        .arg("-O")
        .arg("ReleaseFast")
        .arg(format!("-femit-bin={}", lib.display()));
    if let Some(target) = tc::zig_target() {
        cmd.arg("-target").arg(target);
    }
    cmd.arg(&root);
    tc::run(&mut cmd, "build Zig storage library");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=sheaf_storage");
    println!("cargo:rustc-cfg=sheaf_native_storage");
}