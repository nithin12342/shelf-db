// bump_arena.zig - zero-contention linear arena (spec Pillar 2).
//
// The hot ingestion path never calls malloc. The Rust tier pins a buffer
// (a huge-page slab in production), and this arena carves aligned slices
// from it with a single bump pointer. Allocation is two integer ops plus
// one bounds check. There is no free list and no lock, because each worker
// owns its arena (thread-per-core rule, spec Pillar 4).
//
// The arena struct is extern so the C ABI can pass it by pointer. The
// buffer itself stays owned by the caller for the whole arena lifetime.

const std = @import("std");

pub const SheafArena = extern struct {
    base: ?[*]u8,
    capacity: usize,
    used: usize,
};

/// Bind the arena to a caller buffer. A null buffer yields an empty arena
/// that refuses every allocation instead of crashing.
pub fn init(arena: *SheafArena, buffer: ?[*]u8, capacity: usize) void {
    arena.base = buffer;
    arena.capacity = if (buffer == null) 0 else capacity;
    arena.used = 0;
}

/// Carve `size` bytes at `alignment`. Returns null when the request does
/// not fit or the alignment is not a power of two. The bump pointer only
/// moves forward. Padding bytes between allocations are never reused.
pub fn alloc(arena: *SheafArena, size: usize, alignment: usize) ?[*]u8 {
    const base = arena.base orelse return null;
    if (alignment == 0 or (alignment & (alignment - 1)) != 0) return null;
    if (size == 0) return base; // zero-size reads get the base, no bump

    const base_addr = @intFromPtr(base);
    const current = base_addr + arena.used;
    // Round up to the alignment boundary without division.
    const aligned_addr = (current + alignment - 1) & ~(alignment - 1);
    const pad = aligned_addr - base_addr;
    if (pad > arena.capacity) return null;
    if (size > arena.capacity - pad) return null; // overflow-safe bound
    arena.used = pad + size;
    return @ptrFromInt(aligned_addr);
}

/// Reset the bump pointer. Contents stay in place so pinned readers keep
/// their bytes until the epoch lease retires (the Rust tier owns that rule).
pub fn reset(arena: *SheafArena) void {
    arena.used = 0;
}

pub fn used(arena: *const SheafArena) usize {
    return arena.used;
}

test "arena honors alignment and capacity" {
    var buffer: [256]u8 align(64) = undefined;
    var arena: SheafArena = undefined;
    init(&arena, &buffer, buffer.len);

    const a = alloc(&arena, 10, 1).?;
    try std.testing.expectEqual(@as(usize, 10), used(&arena));
    // The next 64-byte request must skip to the next cache line.
    const b = alloc(&arena, 64, 64).?;
    try std.testing.expectEqual(@as(usize, 0), @intFromPtr(b) % 64);
    try std.testing.expect(@intFromPtr(b) >= @intFromPtr(a) + 10);
    // Exhaustion returns null and does not move the bump pointer.
    const before = used(&arena);
    try std.testing.expect(alloc(&arena, 4096, 8) == null);
    try std.testing.expectEqual(before, used(&arena));
    // Bad alignment is rejected.
    try std.testing.expect(alloc(&arena, 8, 3) == null);

    reset(&arena);
    try std.testing.expectEqual(@as(usize, 0), used(&arena));
}

test "null buffer arena refuses allocation" {
    var arena: SheafArena = undefined;
    init(&arena, null, 1024);
    try std.testing.expect(alloc(&arena, 8, 8) == null);
}