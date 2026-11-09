// exports.zig - the C ABI surface of the Zig storage tier.
//
// Every exported symbol matches native/include/sheaf_storage.h exactly.
// The wrappers only translate pointers and null-check. All logic lives in
// the module files, which keeps the export layer thin and auditable.
//
// Build: `zig build` produces libsheaf_storage.a (see build.zig).
// Test:  `zig build test` runs the module self-tests.

const spine = @import("spine_soa.zig");
const arena = @import("bump_arena.zig");
const block = @import("bcsf_block.zig");

// ---- spine ---------------------------------------------------------------

export fn sheaf_spine_pack(
    out: ?*spine.SheafSpineSoA,
    cell_ids: ?[*]const u32,
    hashes: ?[*]const u8,
    byte_offsets: ?[*]const u64,
    flags_modes: ?[*]const u32,
    count: u32,
) c_int {
    const o = out orelse return -1;
    const ids = cell_ids orelse return -1;
    const h = hashes orelse return -1;
    const off = byte_offsets orelse return -1;
    const fl = flags_modes orelse return -1;
    return if (spine.pack(o, ids, h, off, fl, count)) 0 else -1;
}

export fn sheaf_spine_count(s: ?*const spine.SheafSpineSoA) u32 {
    const p = s orelse return 0;
    return spine.count(p);
}

export fn sheaf_spine_get(
    s: ?*const spine.SheafSpineSoA,
    slot: u32,
    cell_id: ?*u32,
    hash_out: ?[*]u8,
    offset_out: ?*u64,
    flags_out: ?*u32,
) c_int {
    const p = s orelse return -1;
    return if (spine.get(p, slot, cell_id, hash_out, offset_out, flags_out)) 0 else -1;
}

// ---- arena ---------------------------------------------------------------

export fn sheaf_arena_init(a: ?*arena.SheafArena, buffer: ?[*]u8, capacity: usize) void {
    const p = a orelse return;
    arena.init(p, buffer, capacity);
}

export fn sheaf_arena_alloc(a: ?*arena.SheafArena, size: usize, alignment: usize) ?[*]u8 {
    const p = a orelse return null;
    return arena.alloc(p, size, alignment);
}

export fn sheaf_arena_reset(a: ?*arena.SheafArena) void {
    const p = a orelse return;
    arena.reset(p);
}

export fn sheaf_arena_used(a: ?*const arena.SheafArena) usize {
    const p = a orelse return 0;
    return arena.used(p);
}

// ---- BCSF block headers ----------------------------------------------------

export fn sheaf_bcsf_pack_header(
    out: ?*block.SheafBcsfBlockHeader,
    mins: ?[*]const f32,
    maxs: ?[*]const f32,
    dims: u32,
    cell_count: u32,
    payload_offset: u64,
    payload_bytes: u64,
) c_int {
    const o = out orelse return -1;
    const lo = mins orelse return -1;
    const hi = maxs orelse return -1;
    return if (block.packHeader(o, lo, hi, dims, cell_count, payload_offset, payload_bytes)) 0 else -1;
}

export fn sheaf_bcsf_block_matches(
    header: ?*const block.SheafBcsfBlockHeader,
    query_min: ?[*]const f32,
    query_max: ?[*]const f32,
    dims: u32,
) c_int {
    const h = header orelse return 0;
    const lo = query_min orelse return 0;
    const hi = query_max orelse return 0;
    return if (block.blockMatches(h, lo, hi, dims)) 1 else 0;
}