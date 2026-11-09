// build.rs - native toolchain detection and compilation for sheaf-native.
//
// Decision order:
//   1. Feature force-fallback or env SHEAF_NATIVE_DISABLE=1 -> Rust fallback.
//   2. Env SHEAF_KERNEL_CXX set -> use that compiler driver verbatim.
//   3. zig on PATH -> "zig c++" (bundled Clang, works without MSVC/MinGW).
//   4. clang++ or g++ on PATH -> use it with a matching archiver.
//   5. Nothing found -> emit a warning and compile the Rust fallback only.
//
// When a compiler is found, every native/kernel-cpp/src/*.cpp is compiled
// into OUT_DIR and archived as libsheaf_kernel.a. The cfg sheaf_native_kernel
// then activates the FFI adapter in src/kernel/native.rs.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // crates/sheaf-native -> workspace root -> native/
    let native_root = manifest.join("../../native").canonicalize().unwrap_or_else(|_| {
        manifest.join("../../native")
    });
    let kernel_src = native_root.join("kernel-cpp/src");
    let include_dir = native_root.join("include");

    // Rebuild when the native sources or the override variables change.
    println!("cargo:rerun-if-changed={}", kernel_src.display());
    println!("cargo:rerun-if-changed={}", include_dir.display());
    println!("cargo:rerun-if-env-changed=SHEAF_KERNEL_CXX");
    println!("cargo:rerun-if-env-changed=SHEAF_NATIVE_DISABLE");
    // Declare the custom cfg so rustc does not warn about unexpected cfgs.
    println!("cargo:rustc-check-cfg=cfg(sheaf_native_kernel)");

    if env::var("CARGO_FEATURE_FORCE_FALLBACK").is_ok()
        || env::var("SHEAF_NATIVE_DISABLE").map(|v| v == "1").unwrap_or(false)
    {
        println!("cargo:warning=sheaf-native: fallback forced, native kernel not compiled");
        return;
    }

    let Some(driver) = find_compiler() else {
        println!(
            "cargo:warning=sheaf-native: no C++ toolchain found (zig/clang++/g++). \
             Building the pure-Rust fallback. Install a toolchain and rebuild \
             to activate the C++23 kernel."
        );
        return;
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mut objects: Vec<PathBuf> = Vec::new();
    let mut sources: Vec<PathBuf> = std::fs::read_dir(&kernel_src)
        .expect("kernel-cpp/src must exist")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "cpp").unwrap_or(false))
        .collect();
    sources.sort();

    for src in &sources {
        let obj = out_dir.join(format!("{}.o", src.file_stem().unwrap().to_string_lossy()));
        let mut cmd = driver_command(&driver);
        cmd.args(["-O3", "-std=c++23", "-fno-exceptions", "-fno-rtti"])
            .arg("-I")
            .arg(&include_dir)
            .arg("-c")
            .arg(src)
            .arg("-o")
            .arg(&obj);
        run(&mut cmd, "compile kernel source");
        objects.push(obj);
    }

    // Archive the objects. zig ships its own ar. GNU toolchains use ar.
    let lib = out_dir.join("libsheaf_kernel.a");
    let mut ar = if driver.starts_with("zig") {
        let mut c = Command::new("zig");
        c.arg("ar");
        c
    } else {
        Command::new(find_archiver())
    };
    ar.arg("rcs").arg(&lib).args(&objects);
    run(&mut ar, "archive kernel library");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=sheaf_kernel");
    println!("cargo:rustc-cfg=sheaf_native_kernel");
}

// Probe candidate compilers. A candidate counts only if `--version` runs.
fn find_compiler() -> Option<String> {
    if let Ok(custom) = env::var("SHEAF_KERNEL_CXX") {
        if !custom.trim().is_empty() {
            return Some(custom);
        }
    }
    for candidate in ["zig", "clang++", "g++"] {
        let ok = Command::new(candidate)
            .arg(if candidate == "zig" { "version" } else { "--version" })
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return Some(if candidate == "zig" { "zig c++".into() } else { candidate.into() });
        }
    }
    None
}

// Split a driver string like "zig c++" or "C:/llvm/bin/clang++" into a
// Command. The first token is the program, the rest are leading arguments.
fn driver_command(driver: &str) -> Command {
    let mut parts = driver.split_whitespace();
    let mut cmd = Command::new(parts.next().expect("non-empty driver"));
    for p in parts {
        cmd.arg(p);
    }
    cmd
}

fn find_archiver() -> String {
    for candidate in ["llvm-ar", "ar", "gcc-ar"] {
        let ok = Command::new(candidate)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return candidate.into();
        }
    }
    // Last resort: plain ar and let the error surface with a clear message.
    "ar".into()
}

fn run(cmd: &mut Command, what: &str) {
    let out = cmd.output().unwrap_or_else(|e| panic!("{what}: failed to spawn: {e}"));
    if !out.status.success() {
        panic!(
            "{what}: exit {:?}\nstdout:\n{}\nstderr:\n{}",
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}