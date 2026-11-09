// wal.rs - build the C11 write-ahead log library into a static archive.
//
// Sources live in wal-c/src. The translation unit set is:
//   frame.c, epoch_header.c, wal_slab.c, and one platform backend.
// All three platform files compile together safely. Each file guards
// itself with preprocessor tests:
//   platform_posix.c    (!defined(_WIN32))
//   platform_windows.c  (defined(_WIN32))
//   platform_uring.c    (defined(__linux__) and defined(SHEAF_WAL_URING))
// The write symbol then lands in exactly one object per target.
//
// io_uring SQPOLL is selected with -DSHEAF_WAL_URING on Linux targets
// only. Off Linux the define stays absent and the POSIX write path
// below owns the symbol. The uring fallback inside the file still
// works when ring setup fails on a restricted kernel.
//
// Header lookup needs two include roots:
//   -I <native>/include        public contract headers.
//   -I <native>/wal-c          makes "../include/sheaf_wal.h" in
//                              wal_internal.h resolve to native/include.
//
// A missing C toolchain never fails the build. This function prints
// a warning and returns. The Rust side keeps the worker buildable.

use std::env;
use std::path::Path;

use super::toolchain as tc;

pub fn build(native_root: &Path, out_dir: &Path) {
    println!("cargo:rerun-if-env-changed=SHEAF_WAL_CC");

    let Some(driver) = tc::find_c_compiler() else {
        println!(
            "cargo:warning=sheaf-native: no C toolchain found (zig/clang/gcc). \
             The C WAL is not compiled. Install a toolchain and rebuild \
             to activate the durable log."
        );
        return;
    };

    let wal_src = native_root.join("wal-c/src");
    let include_dir = native_root.join("include");
    let wal_root = native_root.join("wal-c");

    // Sorted for a deterministic build. tc::sources_with_ext sorts.
    let sources = tc::sources_with_ext(&wal_src, "c");
    let mut objects = Vec::new();

    // io_uring SQPOLL applies on Linux only. Select it from the
    // TARGET env pair that Cargo provides to build scripts.
    let target = env::var("TARGET").unwrap_or_default();
    let use_uring = target.contains("linux") && !target.contains("windows");

    for src in &sources {
        let obj = out_dir.join(format!(
            "wal_{}.o",
            src.file_stem().unwrap().to_string_lossy()
        ));
        let mut cmd = tc::driver_command(&driver);
        // C11 with optimisation. No exceptions or RTTI concept here.
        // _POSIX_C_SOURCE is set inside platform_posix.c itself.
        cmd.args(["-O2", "-std=c11"]);
        if use_uring {
            cmd.arg("-DSHEAF_WAL_URING");
        }
        // zig bundles MinGW/libc headers. Pin the GNU ABI on Windows
        // so the compile does not depend on a Windows SDK install.
        // x64 calling convention matches MSVC, so the link holds.
        if driver.starts_with("zig") {
            if let Some(triple) = tc::zig_target() {
                cmd.arg("-target").arg(triple);
            }
        }
        // wal_internal.h includes "../include/sheaf_wal.h". The
        // wal-c root turns that into native/include at compile time.
        cmd.arg("-I")
            .arg(&include_dir)
            .arg("-I")
            .arg(&wal_root)
            .arg("-I")
            .arg(&wal_src)
            .arg("-c")
            .arg(src)
            .arg("-o")
            .arg(&obj);
        tc::run(&mut cmd, "compile WAL source");
        objects.push(obj);
    }

    let lib = out_dir.join(tc::archive_name("sheaf_wal"));
    tc::archive(&driver, &objects, &lib);

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=sheaf_wal");
    println!("cargo:rustc-cfg=sheaf_native_wal");
}
