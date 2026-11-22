// page_scan.zig - physical page row ranges in the Zig storage tier (C-04).
//
// The Rust app owns the query predicate and JSON decoding. This module owns
// the physical layout: it walks the spine and returns each row payload offset
// and length. A truncated or overlapping payload is an explicit error.

const std = @import("std");
const spine = @import("spine_soa.zig");

/// Fill offsets and lengths with one entry per live spine slot.
/// Returns the number of rows written, or a negative error code.
pub fn rowRanges(
    s: *const spine.SheafSpineSoA,
    payload: [*]const u8,
    payload_len: usize,
    offsets: [*]u64,
    lengths: [*]u32,
    capacity: u32,
) i32 {
    const count = spine.count(s);
    if (count > capacity) return -1;
    var slot: u32 = 0;
    while (slot < count) : (slot += 1) {
        var offset: u64 = 0;
        if (!spine.get(s, slot, null, null, &offset, null)) return -2;
        const start: usize = @intCast(offset);
        if (start > payload_len or payload_len - start < 4) return -3;
        const header: *const [4]u8 = @ptrCast(payload + start);
        const length: usize = std.mem.readInt(u32, header, .little);
        if (length > payload_len - start - 4) return -4;
        offsets[slot] = offset;
        lengths[slot] = @intCast(length);
    }
    return @intCast(count);
}

test "rowRanges returns offsets and lengths for live slots" {
    // A hand-built spine with two live slots and two length-prefixed rows.
    var s = std.mem.zeroes(spine.SheafSpineSoA);
    s.cell_ids[0] = 0;
    s.cell_ids[1] = 1;
    for (2..spine.SPINE_CELLS) |slot| s.cell_ids[slot] = spine.EMPTY;
    s.byte_offsets[0] = 0;
    s.byte_offsets[1] = 7;
    var payload = [_]u8{ 3, 0, 0, 0, 'a', 'b', 'c', 1, 0, 0, 0, 'z' };
    var offsets: [16]u64 = undefined;
    var lengths: [16]u32 = undefined;
    const n = rowRanges(&s, &payload, payload.len, &offsets, &lengths, 16);
    try std.testing.expectEqual(@as(i32, 2), n);
    try std.testing.expectEqual(@as(u64, 0), offsets[0]);
    try std.testing.expectEqual(@as(u32, 3), lengths[0]);
    try std.testing.expectEqual(@as(u64, 7), offsets[1]);
    try std.testing.expectEqual(@as(u32, 1), lengths[1]);
}
