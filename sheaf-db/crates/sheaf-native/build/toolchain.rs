// toolchain.rs - shared probes and process helpers for the tier builds.
//
// One place owns compiler discovery so the three tiers agree on what
// "zig is installed" means. A candidate counts only when its version
// command runs successfully, which filters broken PATH shims.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// True when the Rust target links with the MSVC linker. MSVC searches
/// for `<name>.lib`, while GNU and macOS search for `lib<name>.a`.
pub fn target_is_msvc() -> bool {
    env::var("TARGET").map(|t| t.contains("windows-msvc")).unwrap_or(false)
}

/// Archive file name that the target linker will find for `-lstatic=<name>`.
pub fn archive_name(name: &str) -> String {
    if target_is_msvc() {
        format!("{name}.lib")
    } else {
        format!("lib{name}.a")
    }
}

/// Zig target triple for the Rust target. On Windows we pin the GNU ABI
/// because zig bundles MinGW headers (windows.h) but not the Windows SDK.
/// The C ABI objects link into an MSVC process unchanged: x64 calling
/// convention is identical, and the tiers use no CRT-dependent types.
pub fn zig_target() -> Option<String> {
    let target = env::var("TARGET").ok()?;
    let mut parts = target.split('-');
    let arch = parts.next()?;
    let arch = match arch {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => return None,
    };
    if target.contains("windows") {
        Some(format!("{arch}-windows-gnu"))
    } else if target.contains("linux") {
        let env_abi = if target.contains("musl") { "musl" } else { "gnu" };
        Some(format!("{arch}-linux-{env_abi}"))
    } else if target.contains("apple") {
        Some(format!("{arch}-macos-none"))
    } else {
        None
    }
}

/// True when `program` runs successfully with `probe_arg`.
pub fn probe(program: &str, probe_arg: &str) -> bool {
    Command::new(program)
        .arg(probe_arg)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Find a C++ driver: explicit override, then zig, clang++, g++.
pub fn find_cxx_compiler() -> Option<String> {
    if let Ok(custom) = env::var("SHEAF_KERNEL_CXX") {
        if !custom.trim().is_empty() {
            return Some(custom);
        }
    }
    if probe("zig", "version") {
        return Some("zig c++".into());
    }
    for candidate in ["clang++", "g++"] {
        if probe(candidate, "--version") {
            return Some(candidate.into());
        }
    }
    None
}

/// Find a C driver: explicit override, then zig, clang, gcc.
pub fn find_c_compiler() -> Option<String> {
    if let Ok(custom) = env::var("SHEAF_WAL_CC") {
        if !custom.trim().is_empty() {
            return Some(custom);
        }
    }
    if probe("zig", "version") {
        return Some("zig cc".into());
    }
    for candidate in ["clang", "gcc"] {
        if probe(candidate, "--version") {
            return Some(candidate.into());
        }
    }
    None
}

/// Split a driver string like "zig c++" into program plus leading args.
pub fn driver_command(driver: &str) -> Command {
    let mut parts = driver.split_whitespace();
    let mut cmd = Command::new(parts.next().expect("non-empty driver"));
    for p in parts {
        cmd.arg(p);
    }
    cmd
}

/// Archive objects into `lib`. zig ships its own ar; GNU uses ar.
pub fn archive(driver: &str, objects: &[PathBuf], lib: &Path) {
    let mut ar = if driver.starts_with("zig") {
        let mut c = Command::new("zig");
        c.arg("ar");
        c
    } else {
        Command::new(find_archiver())
    };
    ar.arg("rcs").arg(lib).args(objects);
    run(&mut ar, "archive native library");
}

fn find_archiver() -> String {
    for candidate in ["llvm-ar", "ar", "gcc-ar"] {
        if probe(candidate, "--version") {
            return candidate.into();
        }
    }
    // Last resort: plain ar, and let the error surface with a clear message.
    "ar".into()
}

/// Run a build command, panicking with full output on failure.
pub fn run(cmd: &mut Command, what: &str) {
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

/// Sorted list of files with the given extension inside `dir`.
pub fn sources_with_ext(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{} must exist: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == ext).unwrap_or(false))
        .collect();
    files.sort();
    files
}