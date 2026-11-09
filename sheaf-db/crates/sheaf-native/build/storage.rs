// storage.rs - build the Zig BCSF storage library into a static archive.
//
// The storage tier ships as one translation unit. `exports.zig` is the
// only entry point. It imports spine_soa.zig, bump_arena.zig, and
// bcsf_block.zig directly, so compiling `exports.zig` compiles the tier.
//
// Build path:
//   zig build-lib exports.zig -static [-target <triple>] -femit-bin=<lib>
//
// The Rust target triple maps to a Zig target with tc::zig_target.
// The archive name follows the linker's convention with
// tc::archive_name, the same rule as the kernel tier.
//
// A missing Zig never fails the build. The function prints a warning
// and returns. The pure-Rust fallback stays active and the workspace
// still builds and tests everywhere.

use std::path::Path;

use super::toolchain as tc;

pub fn build(native_root: &Path, out_dir: &Path) {
    println!("cargo:rerun-if-env-changed=SHEAF_STORAGE_ZIG");

    // Zig only. clang or gcc do not compile Zig modules, so the
    // probe checks for zig exactly. No fallback to a C compiler here.
    if !tc::probe("zig", "version") {
        println!(
            "cargo:warning=sheaf-native: no Zig toolchain found (zig). \
             The Zig storage tier is not compiled. Install Zig and \
             rebuild to activate the BCSF spine and arena."
        );
        return;
    }

    // Entry point of the tier. The module imports do the rest.
    let entry = native_root.join("storage-zig/src/exports.zig");
    if !entry.exists() {
        // Keep the fallback intact when the source tree is incomplete.
        // The warning names the missing entry so the failure is visible.
        println!(
            "cargo:warning=sheaf-native: storage entry not found at {}. \
             The Zig storage tier is skipped and the fallback stays active.",
            entry.display()
        );
        return;
    }

    let lib = out_dir.join(tc::archive_name("sheaf_storage"));

    // zig build-lib emits the whole tier as one archive. -femit-bin
    // pins the exact archive name the linker will search for.
    let mut cmd = tc::driver_command("zig build-lib");
    cmd.arg(&entry);
    cmd.arg("-static");
    cmd.arg("-OReleaseFast");
    if let Some(target) = tc::zig_target() {
        // Pin the target so MinGW headers win on Windows hosts.
        // The C ABI calling convention is identical across the
        // supported targets, so the link into an MSVC process holds.
        cmd.arg("-target");
        cmd.arg(target);
    }
    cmd.arg("-femit-bin");
    cmd.arg(&lib);
    // The export layer links no C library explicitly. -lc keeps the
    // archive self-contained on every host that has Zig installed.
    cmd.arg("-lc");
    tc::run(&mut cmd, "compile Zig storage tier");

    if !lib.exists() {
        println!(
            "cargo:warning=sheaf-native: Zig storage archive was not emitted at {}. \
             The storage tier is skipped and the fallback stays active.",
            lib.display()
        );
        return;
    }

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=sheaf_storage");
    println!("cargo:rustc-cfg=sheaf_native_storage");
}
