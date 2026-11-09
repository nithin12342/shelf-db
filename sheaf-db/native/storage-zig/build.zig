// build.zig - Zig 0.14.x build script for the Tier 2 storage library.
//
// Commands:
//   zig build              -> zig-out/lib/libsheaf_storage.a (static, C ABI)
//   zig build test         -> runs the module self-tests
//   zig build -Doptimize=ReleaseFast
//
// The Rust build.rs invokes `zig build-lib` directly with the same source
// when it integrates the library, so this script and the automated path
// compile identical code.

const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // The static library exposes only the C ABI symbols from exports.zig.
    const lib = b.addStaticLibrary(.{
        .name = "sheaf_storage",
        .root_source_file = b.path("src/exports.zig"),
        .target = target,
        .optimize = optimize,
    });
    lib.linkLibC();
    b.installArtifact(lib);

    // Self-tests cover the spine layout, the arena, and pushdown headers.
    const module_tests = b.addTest(.{
        .root_source_file = b.path("src/tests.zig"),
        .target = target,
        .optimize = optimize,
    });
    const run_tests = b.addRunArtifact(module_tests);
    const test_step = b.step("test", "Run the storage tier self-tests");
    test_step.dependOn(&run_tests.step);
}