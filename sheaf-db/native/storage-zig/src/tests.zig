// tests.zig - test aggregator for the storage tier.
//
// `zig build test` imports every module that carries inline tests. Keeping
// the aggregator separate means a new module joins the gate by adding one
// import line here, without touching the build script.

comptime {
    _ = @import("spine_soa.zig");
    _ = @import("bump_arena.zig");
    _ = @import("bcsf_block.zig");
}