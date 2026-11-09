/*
 * wal_slab.c - orchestrator for the public WAL API (sheaf_wal.h).
 *
 * Composition root of the C WAL. It owns no wire format and no
 * syscalls: frame.c frames records, epoch_header.c codes the A/B
 * slots, and one platform backend moves bytes. This file enforces
 * the durability order and the recovery state machine.
 *
 * Durability order at commit:
 *   1. fsync the log slab (records reach stable storage).
 *   2. Write the INACTIVE epoch slot (the active slot stays intact).
 *   3. fsync the header file (the new epoch becomes the winner).
 * A power cut at any step leaves the previous epoch recoverable.
 *
 * Recovery state machine:
 *   - Pick the valid slot with the highest epoch.
 *   - No valid slot + empty log  -> fresh log, epoch 0.
 *   - No valid slot + filled log -> SHEAF_WAL_ECORRUPT.
 *   - Winner found: the log must hold at least valid_end bytes, and
 *     every record below valid_end must pass framing and sequence
 *     checks. Anything at or beyond valid_end is an uncommitted tail
 *     and is truncated.
 */
#include "wal_internal.h"

/* ---- helpers ----------------------------------------------------------- */

/* Read and decode both slots. Returns the winning slot index (0 or 1)
 * or -1 when neither slot is valid. out receives the winning fields. */
static int pick_winning_slot(SheafFile* header, SheafEpochSlot* out) {
    int64_t size = sheaf_platform_size(header);
    if (size < 0) return -1;

    uint8_t sector[SHEAF_WAL_SLOT_BYTES];
    SheafEpochSlot candidate;
    int winner = -1;
    for (uint32_t i = 0; i < 2; ++i) {
        uint64_t off = (uint64_t)i * SHEAF_WAL_SLOT_BYTES;
        if ((uint64_t)size < off + SHEAF_WAL_SLOT_BYTES) break;
        if (sheaf_platform_read(header, off, sector, sizeof(sector)) != SHEAF_WAL_OK) {
            continue; /* A torn slot read just loses the candidacy. */
        }
        if (sheaf_slot_decode(sector, &candidate) != 0) continue;
        if (winner < 0 || candidate.epoch > out->epoch) {
            *out = candidate;
            winner = (int)i;
        }
    }
    return winner;
}

/* Validate one record at offset. On success fills hdr_out and returns
 * the total record bytes. Returns 0 on corruption or a short tail.
 * payload_buf holds SHEAF_WAL_MAX_PAYLOAD bytes. */
static uint64_t scan_record(SheafFile* log, uint64_t offset, uint64_t limit,
                            uint64_t expected_seq, uint8_t* payload_buf,
                            SheafRecHeader* hdr_out) {
    if (offset + SHEAF_REC_HEADER_BYTES > limit) return 0;
    uint8_t raw[SHEAF_REC_HEADER_BYTES];
    if (sheaf_platform_read(log, offset, raw, sizeof(raw)) != SHEAF_WAL_OK) return 0;

    SheafRecHeader h;
    uint32_t payload_crc;
    if (sheaf_frame_decode(raw, &h, &payload_crc) != 0) return 0;
    if (h.seq != expected_seq) return 0;
    if (h.payload_len > SHEAF_WAL_MAX_PAYLOAD) return 0;
    uint64_t total = SHEAF_REC_HEADER_BYTES + (uint64_t)h.payload_len;
    if (offset + total > limit) return 0;

    if (h.payload_len > 0) {
        if (sheaf_platform_read(log, offset + SHEAF_REC_HEADER_BYTES,
                                payload_buf, h.payload_len) != SHEAF_WAL_OK) {
            return 0;
        }
        if (sheaf_crc32c(payload_buf, h.payload_len) != payload_crc) return 0;
    } else if (payload_crc != sheaf_crc32c(payload_buf, 0)) {
        return 0; /* Empty payload still carries the empty-input CRC. */
    }
    *hdr_out = h;
    return total;
}

/* ---- public API -------------------------------------------------------- */

int sheaf_wal_recover(SheafWal* wal, SheafWalStats* out) {
    if (wal == NULL) return SHEAF_WAL_EINVAL;

    SheafWalStats stats;
    memset(&stats, 0, sizeof(stats));

    SheafEpochSlot slot;
    int winner = pick_winning_slot(wal->header, &slot);
    int64_t log_size = sheaf_platform_size(wal->log);
    if (log_size < 0) return SHEAF_WAL_EIO;

    if (winner < 0) {
        if (log_size > 0) return SHEAF_WAL_ECORRUPT;
        /* Fresh log. The first commit writes slot A. */
        wal->epoch = 0;
        wal->next_seq = 0;
        wal->end = 0;
        wal->next_slot = 0;
        if (out != NULL) *out = stats;
        return SHEAF_WAL_OK;
    }

    /* Committed bytes must exist. The slot was written after the log
     * flush, so a shorter log means storage lost durable data. */
    if ((uint64_t)log_size < slot.valid_end) return SHEAF_WAL_ECORRUPT;

    uint8_t* payload_buf = (uint8_t*)malloc(SHEAF_WAL_MAX_PAYLOAD);
    if (payload_buf == NULL) return SHEAF_WAL_EIO;

    uint64_t offset = 0;
    uint64_t count = 0;
    uint64_t last_seq = 0;
    while (offset < slot.valid_end) {
        SheafRecHeader h;
        uint64_t total = scan_record(wal->log, offset, slot.valid_end,
                                     count, payload_buf, &h);
        if (total == 0) {
            free(payload_buf);
            return SHEAF_WAL_ECORRUPT; /* Damage inside committed range. */
        }
        last_seq = h.seq;
        offset += total;
        count += 1;
    }
    free(payload_buf);

    /* The scan must reproduce the commit's own bookkeeping. */
    if (count > 0 && last_seq != slot.last_seq) return SHEAF_WAL_ECORRUPT;
    if (count == 0 && (slot.last_seq != 0 || slot.valid_end != 0)) {
        return SHEAF_WAL_ECORRUPT;
    }

    stats.truncated_bytes = (uint64_t)log_size - slot.valid_end;
    if (stats.truncated_bytes > 0) {
        if (sheaf_platform_truncate(wal->log, slot.valid_end) != SHEAF_WAL_OK) {
            return SHEAF_WAL_EIO;
        }
        if (sheaf_platform_fsync(wal->log) != SHEAF_WAL_OK) return SHEAF_WAL_EIO;
    }

    wal->epoch = slot.epoch;
    wal->next_seq = count;
    wal->end = slot.valid_end;
    wal->next_slot = (winner == 0) ? 1u : 0u;

    stats.epoch = slot.epoch;
    stats.last_seq = (count > 0) ? last_seq : 0;
    stats.valid_end = slot.valid_end;
    stats.records_recovered = count;
    if (out != NULL) *out = stats;
    return SHEAF_WAL_OK;
}

SheafWal* sheaf_wal_open(const char* log_path, const char* header_path) {
    if (log_path == NULL || header_path == NULL) return NULL;
    SheafWal* wal = (SheafWal*)malloc(sizeof(SheafWal));
    if (wal == NULL) return NULL;
    memset(wal, 0, sizeof(*wal));

    wal->log = sheaf_platform_open(log_path);
    if (wal->log == NULL) goto fail;
    wal->header = sheaf_platform_open(header_path);
    if (wal->header == NULL) goto fail;
    if (sheaf_wal_recover(wal, NULL) != SHEAF_WAL_OK) goto fail;
    return wal;

fail:
    if (wal->log != NULL) sheaf_platform_close(wal->log);
    if (wal->header != NULL) sheaf_platform_close(wal->header);
    free(wal);
    return NULL;
}

int sheaf_wal_append(SheafWal* wal, uint64_t epoch,
                     const uint8_t* payload, uint32_t len) {
    if (wal == NULL) return SHEAF_WAL_EINVAL;
    if (len > SHEAF_WAL_MAX_PAYLOAD) return SHEAF_WAL_EINVAL;
    if (len > 0 && payload == NULL) return SHEAF_WAL_EINVAL;

    SheafRecHeader h;
    h.seq = wal->next_seq;
    h.epoch = epoch;
    h.payload_len = len;

    uint8_t raw[SHEAF_REC_HEADER_BYTES];
    sheaf_frame_encode(raw, &h, payload);
    if (sheaf_platform_write(wal->log, wal->end, raw, sizeof(raw)) != SHEAF_WAL_OK) {
        return SHEAF_WAL_EIO;
    }
    if (len > 0) {
        if (sheaf_platform_write(wal->log, wal->end + SHEAF_REC_HEADER_BYTES,
                                 payload, len) != SHEAF_WAL_OK) {
            return SHEAF_WAL_EIO;
        }
    }
    wal->end += SHEAF_REC_HEADER_BYTES + (uint64_t)len;
    wal->next_seq += 1;
    return SHEAF_WAL_OK;
}

int sheaf_wal_commit_epoch(SheafWal* wal, uint64_t new_epoch) {
    if (wal == NULL) return SHEAF_WAL_EINVAL;
    if (new_epoch <= wal->epoch) return SHEAF_WAL_EINVAL;

    /* Step 1: the records must be durable before the slot names them. */
    if (sheaf_platform_fsync(wal->log) != SHEAF_WAL_OK) return SHEAF_WAL_EIO;

    /* Step 2: write the inactive slot. The active slot still names the
     * previous epoch if power fails during this write. */
    SheafEpochSlot slot;
    slot.epoch = new_epoch;
    slot.valid_end = wal->end;
    slot.last_seq = (wal->next_seq == 0) ? 0 : wal->next_seq - 1;
    uint8_t sector[SHEAF_WAL_SLOT_BYTES];
    sheaf_slot_encode(sector, &slot);
    uint64_t off = (uint64_t)wal->next_slot * SHEAF_WAL_SLOT_BYTES;
    if (sheaf_platform_write(wal->header, off, sector, sizeof(sector)) != SHEAF_WAL_OK) {
        return SHEAF_WAL_EIO;
    }

    /* Step 3: the barrier that makes the new epoch the winner. */
    if (sheaf_platform_fsync(wal->header) != SHEAF_WAL_OK) return SHEAF_WAL_EIO;

    wal->epoch = new_epoch;
    wal->next_slot ^= 1u;
    return SHEAF_WAL_OK;
}

int sheaf_wal_flush(SheafWal* wal) {
    if (wal == NULL) return SHEAF_WAL_EINVAL;
    return sheaf_platform_fsync(wal->log);
}

uint64_t sheaf_wal_epoch(const SheafWal* wal) {
    return (wal != NULL) ? wal->epoch : 0;
}

void sheaf_wal_close(SheafWal* wal) {
    if (wal == NULL) return;
    (void)sheaf_platform_fsync(wal->log);
    sheaf_platform_close(wal->log);
    sheaf_platform_close(wal->header);
    free(wal);
}