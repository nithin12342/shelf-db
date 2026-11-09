// kernel.rs - build the C++23 math kernel into a static library.
//
// Every native/kernel-cpp/src/*.cpp compiles with -O3 -std=c++23 and no
// exceptions or RTTI. Runtime CPUID dispatch owns the ISA choice, so no
// -march flag appears here. The archive name follows the target linker
// convention (sheaf_kernel.lib for MSVC, libsheaf_kernel.a elsewhere).

use std::path::Path;

use super::toolchain as tc;

pub fn build(native_root: &Path, out_dir: &Path) {
    println!("cargo:rerun-if-env-changed=SHEAF_KERNEL_CXX");

    let Some(driver) = tc::find_cxx_compiler() else {
        println!(
            "cargo:warning=sheaf-native: no C++ toolchain found (zig/clang++/g++). \
             The pure-Rust kernel fallback stays active. Install a toolchain \
             and rebuild to activate the C++23 kernel."
        );
        return;
    };

    let kernel_src = native_root.join("kernel-cpp/src");
    let include_dir = native_root.join("include");
    let sources = tc::sources_with_ext(&kernel_src, "cpp");
    let mut objects = Vec::new();

    for src in &sources {
        let obj = out_dir.join(format!("kernel_{}.o", src.file_stem().unwrap().to_string_lossy()));
        let mut cmd = tc::driver_command(&driver);
        cmd.args(["-O3", "-std=c++23", "-fno-exceptions", "-fno-rtti"]);
        // zig bundles MinGW/libc++ headers. Pin the GNU ABI on Windows so
        // the compile does not depend on a Windows SDK installation.
        if driver.starts_with("zig") {
            if let Some(target) = tc::zig_target() {
                cmd.arg("-target").arg(target);
            }
        }
        cmd.arg("-I")
            .arg(&include_dir)
            .arg("-c")
            .arg(src)
            .arg("-o")
            .arg(&obj);
        tc::run(&mut cmd, "compile kernel source");
        objects.push(obj);
    }

    let lib = out_dir.join(tc::archive_name("sheaf_kernel"));
    tc::archive(&driver, &objects, &lib);

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=sheaf_kernel");
    println!("cargo:rustc-cfg=sheaf_native_kernel");
}