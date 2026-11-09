// spine_soa.zig - the SheafSpineSoA layout (spec Pillar 2).
//
// The spine flattens cell topology into four parallel arrays. Each array
// starts on a 64-byte boundary, so a scan of cell ids never drags hash or
// offset bytes into L1. The whole block is 512 bytes = eight cache lines.
//
// Layout contract (must match native/include/sheaf_storage.h):
//   cell_ids      16 x u32   =  64 bytes, offset   0
//   stalk_hashes  16 x 16B   = 256 bytes, offset  64
//   byte_offsets  16 x u64   = 128 bytes, offset 320
//   flags_modes   16 x u32   =  64 bytes, offset 448
//
// This file targets Zig 0.14.x. The comptime block below turns any layout
// drift into a compile error instead of a silent ABI break.

const std = @import("std");

pub const SPINE_CELLS: u32 = 16;
pub const HASH_BYTES: u32 = 16;
pub const EMPTY: u32 = 0xFFFFFFFF;

pub const SheafSpineSoA = extern struct {
    cell_ids: [SPINE_CELLS]u32 align(64),
    stalk_hashes: [SPINE_CELLS][HASH_BYTES]u8 align(64),
    byte_offsets: [SPINE_CELLS]u64 align(64),
    flags_modes: [SPINE_CELLS]u32 align(64),
};

comptime {
    if (@sizeOf(SheafSpineSoA) != 512) {
        @compileError("SheafSpineSoA must stay exactly 512 bytes");
    }
    if (@alignOf(SheafSpineSoA) != 64) {
        @compileError("SheafSpineSoA must stay 64-byte aligned");
    }
    if (@offsetOf(SheafSpineSoA, "stalk_hashes") != 64) {
        @compileError("stalk_hashes must start at cache line 1");
    }
    if (@offsetOf(SheafSpineSoA, "byte_offsets") != 320) {
        @compileError("byte_offsets must start at byte 320");
    }
    if (@offsetOf(SheafSpineSoA, "flags_modes") != 448) {
        @compileError("flags_modes must start at cache line 7");
    }
}

/// Pack parallel arrays into the spine. Slots beyond count get the EMPTY
/// sentinel so readers can find the end without a separate length field.
/// Returns false when count exceeds the block capacity.
pub fn pack(
    out: *SheafSpineSoA,
    cell_ids: [*]const u32,
    hashes: [*]const u8,
    offsets: [*]const u64,
    flags: [*]const u32,
    n_cells: u32,
) bool {
    if (n_cells > SPINE_CELLS) return false;
    var i: u32 = 0;
    while (i < n_cells) : (i += 1) {
        out.cell_ids[i] = cell_ids[i];
        const src = (hashes + i * HASH_BYTES)[0..HASH_BYTES];
        @memcpy(&out.stalk_hashes[i], src);
        out.byte_offsets[i] = offsets[i];
        out.flags_modes[i] = flags[i];
    }
    // Mark the tail empty and zero it so hashes never leak stale bytes.
    while (i < SPINE_CELLS) : (i += 1) {
        out.cell_ids[i] = EMPTY;
        @memset(&out.stalk_hashes[i], 0);
        out.byte_offsets[i] = 0;
        out.flags_modes[i] = 0;
    }
    return true;
}

/// Number of populated slots, stopping at the first sentinel.
pub fn count(spine: *const SheafSpineSoA) u32 {
    var n: u32 = 0;
    while (n < SPINE_CELLS) : (n += 1) {
        if (spine.cell_ids[n] == EMPTY) break;
    }
    return n;
}

/// Read one slot. Returns false for an empty or out-of-range slot.
pub fn get(
    spine: *const SheafSpineSoA,
    slot: u32,
    cell_id: ?*u32,
    hash_out: ?[*]u8,
    offset_out: ?*u64,
    flags_out: ?*u32,
) bool {
    if (slot >= SPINE_CELLS) return false;
    if (spine.cell_ids[slot] == EMPTY) return false;
    if (cell_id) |p| p.* = spine.cell_ids[slot];
    if (hash_out) |p| @memcpy(p[0..HASH_BYTES], &spine.stalk_hashes[slot]);
    if (offset_out) |p| p.* = spine.byte_offsets[slot];
    if (flags_out) |p| p.* = spine.flags_modes[slot];
    return true;
}

// Layout self-test, runnable with `zig test src/spine_soa.zig`.
test "spine packs and reads back all slots" {
    var spine: SheafSpineSoA = undefined;
    var ids: [SPINE_CELLS]u32 = undefined;
    var hashes: [SPINE_CELLS * HASH_BYTES]u8 = undefined;
    var offsets: [SPINE_CELLS]u64 = undefined;
    var flags: [SPINE_CELLS]u32 = undefined;
    for (&ids, 0..) |*id, i| {
        id.* = @intCast(100 + i);
        offsets[i] = @intCast(i * 64);
        flags[i] = @intCast(i);
        for (hashes[i * HASH_BYTES ..][0..HASH_BYTES], 0..) |*b, j| {
            b.* = @intCast((i + j) % 256);
        }
    }
    try std.testing.expect(pack(&spine, &ids, &hashes, &offsets, &flags, 5));
    try std.testing.expectEqual(@as(u32, 5), count(&spine));
    var id: u32 = 0;
    var off: u64 = 0;
    try std.testing.expect(get(&spine, 3, &id, null, &off, null));
    try std.testing.expectEqual(@as(u32, 103), id);
    try std.testing.expectEqual(@as(u64, 192), off);
    try std.testing.expect(!get(&spine, 5, &id, null, null, null));
    try std.testing.expect(!pack(&spine, &ids, &hashes, &offsets, &flags, 17));
}