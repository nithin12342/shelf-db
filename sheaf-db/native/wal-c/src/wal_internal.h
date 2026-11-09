/*
 * wal_internal.h - shared internals for the WAL translation unit set.
 *
 * One platform file compiles per build and implements the file interface
 * below. frame.c owns record framing, epoch_header.c owns the A/B slots,
 * and wal_slab.c orchestrates the public API. No other file includes this.
 */
#ifndef SHEAF_WAL_INTERNAL_H
#define SHEAF_WAL_INTERNAL_H

#include "../include/sheaf_wal.h"

#include <stdlib.h>
#include <string.h>

/* ---- platform file interface ------------------------------------------- */

typedef struct SheafFile SheafFile;

/* Open or create. Returns null on failure. */
SheafFile* sheaf_platform_open(const char* path);
/* Exact-length positioned write. Returns 0 or SHEAF_WAL_EIO. */
int sheaf_platform_write(SheafFile* f, uint64_t offset, const void* buf, size_t len);
/* Exact-length positioned read. Short reads return SHEAF_WAL_EIO; the
 * caller checks the file size first when a short read is expected. */
int sheaf_platform_read(SheafFile* f, uint64_t offset, void* buf, size_t len);
/* Durability barrier. */
int sheaf_platform_fsync(SheafFile* f);
/* File size in bytes, or -1 on failure. */
int64_t sheaf_platform_size(SheafFile* f);
/* Shrink or extend to len. */
int sheaf_platform_truncate(SheafFile* f, uint64_t len);
void sheaf_platform_close(SheafFile* f);

/* ---- record framing (frame.c) ------------------------------------------ */

#define SHEAF_REC_MAGIC 0x4C574853u /* "SHWL" */
#define SHEAF_REC_HEADER_BYTES 32u

/*
 * Record header wire layout (little-endian, 32 bytes):
 *   [0..4)   magic
 *   [4..12)  seq          monotonic record sequence
 *   [12..20) epoch        epoch supplied by the writer
 *   [20..24) payload_len
 *   [24..28) payload_crc  CRC32C of the payload bytes
 *   [28..32) header_crc   CRC32C of bytes [0..28)
 */
typedef struct SheafRecHeader {
    uint64_t seq;
    uint64_t epoch;
    uint32_t payload_len;
} SheafRecHeader;

uint32_t sheaf_crc32c(const uint8_t* data, size_t len);
/* Serialize a header into 32 bytes. payload_crc covers the payload. */
void sheaf_frame_encode(uint8_t out[32], const SheafRecHeader* h,
                        const uint8_t* payload);
/* Validate magic + header crc. Returns 0 on success, -1 on corruption. */
int sheaf_frame_decode(const uint8_t in[32], SheafRecHeader* out,
                       uint32_t* payload_crc);

/* ---- epoch header slots (epoch_header.c) -------------------------------- */

#define SHEAF_SLOT_MAGIC 0x44454853u /* "SHED" */
#define SHEAF_SLOT_VERSION 1u

/*
 * Slot wire layout inside one 4096-byte sector:
 *   [0..4)   magic
 *   [4..8)   format version
 *   [8..16)  epoch
 *   [16..24) valid_end     log bytes covered by this commit
 *   [24..32) last_seq      highest record sequence at commit time
 *   [32..36) crc32c of bytes [0..32)
 *   [36..4096) zero padding
 */
typedef struct SheafEpochSlot {
    uint64_t epoch;
    uint64_t valid_end;
    uint64_t last_seq;
} SheafEpochSlot;

/* Serialize into a full zeroed 4096-byte sector buffer. */
void sheaf_slot_encode(uint8_t* sector, const SheafEpochSlot* slot);
/* Returns 0 when magic, version, and crc all pass. */
int sheaf_slot_decode(const uint8_t* sector, SheafEpochSlot* out);

/* ---- orchestrator state (wal_slab.c) ------------------------------------ */

struct SheafWal {
    SheafFile* log;
    SheafFile* header;
    uint64_t epoch;      /* committed epoch from the winning slot   */
    uint64_t next_seq;   /* sequence for the next appended record   */
    uint64_t end;        /* append offset (== valid_end at commit)  */
    uint32_t next_slot;  /* 0 = write A next, 1 = write B next      */
};

#endif /* SHEAF_WAL_INTERNAL_H */