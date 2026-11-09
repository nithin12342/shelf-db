// bcsf_block.zig - BCSF block headers and predicate pushdown (Pillar 2).
//
// Every BCSF block carries a 64-byte header with a coordinate bounding
// box. The reader compares the query box against the header BEFORE reading
// payload bytes from disk. Non-matching blocks cost one cache line, not a
// disk seek. This is the offset-driven pushdown path from the spec.
//
// The header struct is extern and must match sheaf_storage.h byte for byte.

const std = @import("std");

pub const COORD_DIMS: u32 = 4;
pub const MAGIC: u32 = 0x46534342; // "BCSF" little-endian

pub const SheafBcsfBlockHeader = extern struct {
    magic: u32,
    cell_count: u32,
    coord_min: [COORD_DIMS]f32,
    coord_max: [COORD_DIMS]f32,
    payload_offset: u64,
    payload_bytes: u64,
    dims: u32,
    reserved: u32,
};

comptime {
    if (@sizeOf(SheafBcsfBlockHeader) != 64) {
        @compileError("BCSF block header must stay exactly 64 bytes");
    }
}

/// Fill a header from per-dimension bounds. Unused dimensions get an
/// inverted box (-inf, +inf) so they can never reject a query by accident.
pub fn packHeader(
    out: *SheafBcsfBlockHeader,
    mins: [*]const f32,
    maxs: [*]const f32,
    dims: u32,
    cell_count: u32,
    payload_offset: u64,
    payload_bytes: u64,
) bool {
    if (dims == 0 or dims > COORD_DIMS) return false;
    out.* = .{
        .magic = MAGIC,
        .cell_count = cell_count,
        .coord_min = undefined,
        .coord_max = undefined,
        .payload_offset = payload_offset,
        .payload_bytes = payload_bytes,
        .dims = dims,
        .reserved = 0,
    };
    var d: u32 = 0;
    while (d < COORD_DIMS) : (d += 1) {
        if (d < dims) {
            if (mins[d] > maxs[d]) return false; // inverted input box
            out.coord_min[d] = mins[d];
            out.coord_max[d] = maxs[d];
        } else {
            out.coord_min[d] = -std.math.inf(f32);
            out.coord_max[d] = std.math.inf(f32);
        }
    }
    return true;
}

/// True when the query box intersects the block box on every active
/// dimension. Touching boundaries count as a match (closed intervals).
/// An invalid header never matches, so corrupt sidecars fail closed.
pub fn blockMatches(
    header: *const SheafBcsfBlockHeader,
    query_min: [*]const f32,
    query_max: [*]const f32,
    dims: u32,
) bool {
    if (header.magic != MAGIC) return false;
    if (header.dims == 0 or header.dims > COORD_DIMS) return false;
    if (dims != header.dims) return false;
    var d: u32 = 0;
    while (d < dims) : (d += 1) {
        // Disjoint on this axis when one box ends before the other starts.
        if (query_max[d] < header.coord_min[d]) return false;
        if (query_min[d] > header.coord_max[d]) return false;
    }
    return true;
}

test "pushdown accepts intersecting boxes and rejects disjoint ones" {
    var header: SheafBcsfBlockHeader = undefined;
    const mins = [_]f32{ 0.0, 10.0 };
    const maxs = [_]f32{ 5.0, 20.0 };
    try std.testing.expect(packHeader(&header, &mins, &maxs, 2, 16, 4096, 8192));

    const hit_min = [_]f32{ 4.0, 19.0 };
    const hit_max = [_]f32{ 9.0, 30.0 };
    try std.testing.expect(blockMatches(&header, &hit_min, &hit_max, 2));

    const miss_min = [_]f32{ 6.0, 10.0 };
    const miss_max = [_]f32{ 9.0, 20.0 };
    try std.testing.expect(!blockMatches(&header, &miss_min, &miss_max, 2));

    // Corrupt magic fails closed.
    header.magic = 0;
    try std.testing.expect(!blockMatches(&header, &hit_min, &hit_max, 2));
}