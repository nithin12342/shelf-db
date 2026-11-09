/*
 * epoch_header.c - double-buffered alternating epoch slots (spec Pillar 3).
 *
 * The header file holds two 4096-byte sectors: slot A at offset 0 and
 * slot B at offset 4096. A commit writes the INACTIVE slot, never the one
 * that currently defines the state. If power drops mid-write, the active
 * slot stays untouched and valid. Recovery picks the valid slot with the
 * highest epoch, which is exactly the A/B toggle invariant.
 *
 * The codec here only encodes and validates one sector. The orchestrator
 * in wal_slab.c decides which slot to write and which one wins.
 */
#include "wal_internal.h"

void sheaf_slot_encode(uint8_t* sector, const SheafEpochSlot* slot) {
    /* Zero the whole sector first so padding never carries stale bytes.
     * The zeroed tail also makes torn-write detection unambiguous. */
    memset(sector, 0, SHEAF_WAL_SLOT_BYTES);
    put_u32(sector, SHEAF_SLOT_MAGIC);
    put_u32(sector + 4, SHEAF_SLOT_VERSION);
    put_u64(sector + 8, slot->epoch);
    put_u64(sector + 16, slot->valid_end);
    put_u64(sector + 24, slot->last_seq);
    put_u32(sector + 32, sheaf_crc32c(sector, 32));
}

int sheaf_slot_decode(const uint8_t* sector, SheafEpochSlot* out) {
    if (get_u32(sector) != SHEAF_SLOT_MAGIC) return -1;
    if (get_u32(sector + 4) != SHEAF_SLOT_VERSION) return -1;
    if (get_u32(sector + 32) != sheaf_crc32c(sector, 32)) return -1;
    out->epoch = get_u64(sector + 8);
    out->valid_end = get_u64(sector + 16);
    out->last_seq = get_u64(sector + 24);
    return 0;
}