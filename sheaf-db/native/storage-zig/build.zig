// build.zig - Zig 0.17.x build script for the Tier 2 storage library.
//
// Commands:
//   zig build              -> zig-out/lib/sheaf_storage.lib (static, C ABI)
//   zig build test         -> runs the module self-tests
//   zig build -Doptimize=ReleaseFast
//
// The Rust build.rs invokes `zig build-lib` directly with the same source
// when it integrates the library, so this script and the automated path
// compile identical code.
//
// Zig 0.17 moved compilation options into a root module. addStaticLibrary
// and the flat addTest options are gone. createModule holds the source,
// target, and optimize, and addLibrary/addTest take that module.

const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // The static library exposes only the C ABI symbols from exports.zig.
    const lib_module = b.createModule(.{
        .root_source_file = b.path("src/exports.zig"),
        .target = target,
        .optimize = optimize,
    });
    lib_module.link_libc = true;
    const lib = b.addLibrary(.{
        .name = "sheaf_storage",
        .root_module = lib_module,
        .linkage = .static,
    });
    b.installArtifact(lib);

    // Self-tests cover the spine layout, the arena, and pushdown headers.
    const test_module = b.createModule(.{
        .root_source_file = b.path("src/tests.zig"),
        .target = target,
        .optimize = optimize,
    });
    const module_tests = b.addTest(.{ .root_module = test_module });
    const run_tests = b.addRunArtifact(module_tests);
    const test_step = b.step("test", "Run the storage tier self-tests");
    test_step.dependOn(&run_tests.step);
}