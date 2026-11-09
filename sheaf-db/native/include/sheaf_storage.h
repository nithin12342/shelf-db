/*
 * sheaf_storage.h - C ABI contract between Rust orchestration (Tier 3) and
 * the Zig physical storage tier (Tier 2, spec Pillar 2).
 *
 * Three objects cross the boundary:
 *   1. SheafSpineSoA  - the 64-byte aligned struct-of-arrays spine.
 *   2. SheafArena     - a bump allocator over a caller-owned buffer.
 *   3. SheafBcsfBlockHeader - coordinate bounds for predicate pushdown.
 *
 * The Zig side never calls a global allocator. Every byte comes from a
 * buffer the Rust tier pinned, which keeps the affine-lease rule intact.
 */
#ifndef SHEAF_STORAGE_H
#define SHEAF_STORAGE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SHEAF_SPINE_CELLS 16u   /* cells per spine block                    */
#define SHEAF_HASH_BYTES  16u   /* BLAKE3-128 digest width                  */
#define SHEAF_COORD_DIMS  4u    /* bounding-box dimensions in a block header*/
#define SHEAF_BCSF_MAGIC  0x46534342u /* "BCSF" little-endian               */

/*
 * Struct-of-arrays spine, exactly 512 bytes = eight 64-byte cache lines.
 * Field order is part of the ABI. Each field starts on a cache-line
 * boundary so a reader touching one array never pulls another into L1.
 *
 * Note: the specification sketch lists [4][16] hashes and [8] offsets.
 * This build uses one 16-byte hash and one offset per cell so all 16
 * slots are addressable. The layout stays 64-byte aligned throughout.
 */
typedef struct SheafSpineSoA {
    uint32_t cell_ids[SHEAF_SPINE_CELLS];                 /*  64 bytes */
    uint8_t  stalk_hashes[SHEAF_SPINE_CELLS][SHEAF_HASH_BYTES]; /* 256 bytes */
    uint64_t byte_offsets[SHEAF_SPINE_CELLS];             /* 128 bytes */
    uint32_t flags_modes[SHEAF_SPINE_CELLS];              /*  64 bytes */
} SheafSpineSoA;

/* Sentinel marking an unused spine slot. */
#define SHEAF_SPINE_EMPTY 0xFFFFFFFFu

/* Bump arena over caller memory. No system allocator is involved. */
typedef struct SheafArena {
    uint8_t* base;      /* start of the pinned buffer   */
    size_t capacity;    /* buffer length in bytes     */
    size_t used;        /* bytes consumed so far      */
} SheafArena;

/* Initialize an arena over a buffer. Alignment of base is the caller's job. */
void sheaf_arena_init(SheafArena* arena, void* buffer, size_t capacity);

/*
 * Carve size bytes at the requested alignment. Returns null when the
 * buffer is exhausted. Alignment must be a power of two.
 */
uint8_t* sheaf_arena_alloc(SheafArena* arena, size_t size, size_t alignment);

/* Reset the bump pointer. The buffer contents are untouched. */
void sheaf_arena_reset(SheafArena* arena);

/* Bytes consumed so far. */
size_t sheaf_arena_used(const SheafArena* arena);

/*
 * Pack parallel arrays into one spine block. count must be <= 16.
 * hashes points to count * 16 bytes. Unused slots become SHEAF_SPINE_EMPTY.
 * Returns 0 on success, -1 on a bad count or null pointer.
 */
int sheaf_spine_pack(SheafSpineSoA* out, const uint32_t* cell_ids,
                     const uint8_t* hashes, const uint64_t* byte_offsets,
                     const uint32_t* flags_modes, uint32_t count);

/* Number of populated slots (stops at the first sentinel). */
uint32_t sheaf_spine_count(const SheafSpineSoA* spine);

/*
 * Read one slot. Any output pointer can be null to skip that field.
 * Returns 0 on success, -1 when the slot is empty or out of range.
 */
int sheaf_spine_get(const SheafSpineSoA* spine, uint32_t slot,
                    uint32_t* cell_id, uint8_t* hash_out,
                    uint64_t* offset_out, uint32_t* flags_out);

/*
 * 64-byte BCSF block header. The coordinate bounding box lets the reader
 * reject a block before touching its payload (offset-driven pushdown).
 */
typedef struct SheafBcsfBlockHeader {
    uint32_t magic;                     /* SHEAF_BCSF_MAGIC            */
    uint32_t cell_count;                /* cells in the block payload  */
    float coord_min[SHEAF_COORD_DIMS];  /* per-dimension lower bounds  */
    float coord_max[SHEAF_COORD_DIMS];  /* per-dimension upper bounds  */
    uint64_t payload_offset;            /* byte offset of the payload  */
    uint64_t payload_bytes;             /* payload length              */
    uint32_t dims;                      /* active dimensions (<= 4)    */
    uint32_t reserved;                  /* zero                        */
} SheafBcsfBlockHeader;

/* Fill a header. Returns 0 on success, -1 on bad dims or null pointers. */
int sheaf_bcsf_pack_header(SheafBcsfBlockHeader* out, const float* mins,
                           const float* maxs, uint32_t dims,
                           uint32_t cell_count, uint64_t payload_offset,
                           uint64_t payload_bytes);

/*
 * Predicate pushdown: returns 1 when the query box intersects the block
 * box on every active dimension, 0 otherwise. An invalid header returns 0.
 */
int sheaf_bcsf_block_matches(const SheafBcsfBlockHeader* header,
                             const float* query_min, const float* query_max,
                             uint32_t dims);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SHEAF_STORAGE_H */