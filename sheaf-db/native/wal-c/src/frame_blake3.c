/*
 * frame_blake3.c - BLAKE3 frame mode for the WAL (spec Pillar 3, line 241).
 *
 * Timestamp: 2026-11-22_12-48-26 +05:30
 * Remote: origin https://github.com/nithin12342/shelf-db.git
 * Branch: combined-stack-feature-improvement
 * Base commit: 3832065581f3ff4240f4d267c38bcde8fea1c24e
 *
 * Spec line 241 requires every framed append record to carry a BLAKE3
 * checksum. This file adds that record mode. CRC32C stays in frame.c as the
 * fast sector and header check; this file does not replace it.
 *
 * The digest below is a compact C11 implementation of BLAKE3-256. The IV,
 * the seven message schedules, the round function, and the tree layout come
 * from the official BLAKE3 reference implementation (BLAKE3-team/BLAKE3, c/
 * directory, CC0 public domain). This translation keeps the portable scalar
 * path only. It drops keyed hashing, derive-key, and SIMD dispatch. It returns
 * the same 32-byte digest as the reference for every input length.
 *
 * Record header layout (little-endian, 60 bytes), BLAKE3 mode:
 *   [0..4)   magic "SHWL"
 *   [4..12)  seq             monotonic record sequence
 *   [12..20) epoch           writer epoch
 *   [20..24) payload_len
 *   [24..56) BLAKE3-256 of the payload
 *   [56..60) CRC32C of bytes [0..56)
 */
#include "wal_internal.h"

/* ---- compact BLAKE3-256 -------------------------------------------------- */

#define B3_BLOCK_LEN 64u
#define B3_CHUNK_LEN 1024u
#define B3_OUT_LEN 32u

enum b3_flags {
    B3_CHUNK_START = 1u << 0,
    B3_CHUNK_END   = 1u << 1,
    B3_PARENT      = 1u << 2,
    B3_ROOT        = 1u << 3
};

static const uint32_t B3_IV[8] = {
    0x6A09E667u, 0xBB67AE85u, 0x3C6EF372u, 0xA54FF53Au,
    0x510E527Fu, 0x9B05688Cu, 0x1F83D9ABu, 0x5BE0CD19u
};

/* Row 0 is identity. Each later row is the prior row put through the BLAKE3
 * message permutation. Seven rows give the seven compression rounds. */
static const uint8_t B3_SCHEDULE[7][16] = {
    {  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 15 },
    {  2,  6,  3, 10,  7,  0,  4, 13,  1, 11, 12,  5,  9, 14, 15,  8 },
    {  3,  4, 10, 12, 13,  2,  7, 14,  6,  5,  9,  0, 11, 15,  8,  1 },
    { 10,  7, 12,  9, 14,  3, 13, 15,  4,  0, 11,  2,  5,  8,  1,  6 },
    { 12, 13,  9, 11, 15, 10, 14,  8,  7,  2,  5,  3,  0,  1,  6,  4 },
    {  9, 14, 11,  5,  8, 12, 15,  1, 13,  3,  0, 10,  2,  6,  4,  7 },
    { 11, 15,  5,  0,  1,  9,  8,  6, 14, 10,  2, 12,  3,  4,  7, 13 }
};

static uint32_t b3_load32(const uint8_t* p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static void b3_store32(uint8_t* p, uint32_t w) {
    p[0] = (uint8_t)w;
    p[1] = (uint8_t)(w >> 8);
    p[2] = (uint8_t)(w >> 16);
    p[3] = (uint8_t)(w >> 24);
}

static uint32_t b3_rotr32(uint32_t w, unsigned c) {
    return (w >> c) | (w << (32u - c));
}

/* The BLAKE2s-shaped mixing function shared by every BLAKE3 round. */
static void b3_g(uint32_t s[16], unsigned a, unsigned b, unsigned c,
                 unsigned d, uint32_t x, uint32_t y) {
    s[a] = s[a] + s[b] + x;
    s[d] = b3_rotr32(s[d] ^ s[a], 16u);
    s[c] = s[c] + s[d];
    s[b] = b3_rotr32(s[b] ^ s[c], 12u);
    s[a] = s[a] + s[b] + y;
    s[d] = b3_rotr32(s[d] ^ s[a], 8u);
    s[c] = s[c] + s[d];
    s[b] = b3_rotr32(s[b] ^ s[c], 7u);
}

static void b3_compress_pre(uint32_t state[16], const uint32_t cv[8],
                            const uint8_t block[B3_BLOCK_LEN],
                            uint32_t block_len, uint64_t counter,
                            uint32_t flags) {
    uint32_t m[16];
    for (unsigned i = 0; i < 16; ++i) m[i] = b3_load32(block + 4u * i);
    for (unsigned i = 0; i < 8; ++i) state[i] = cv[i];
    for (unsigned i = 0; i < 4; ++i) state[8u + i] = B3_IV[i];
    state[12] = (uint32_t)counter;
    state[13] = (uint32_t)(counter >> 32);
    state[14] = block_len;
    state[15] = flags;
    for (unsigned r = 0; r < 7; ++r) {
        const uint8_t* s = B3_SCHEDULE[r];
        b3_g(state,  0,  4,  8, 12, m[s[0]],  m[s[1]]);
        b3_g(state,  1,  5,  9, 13, m[s[2]],  m[s[3]]);
        b3_g(state,  2,  6, 10, 14, m[s[4]],  m[s[5]]);
        b3_g(state,  3,  7, 11, 15, m[s[6]],  m[s[7]]);
        b3_g(state,  0,  5, 10, 15, m[s[8]],  m[s[9]]);
        b3_g(state,  1,  6, 11, 12, m[s[10]], m[s[11]]);
        b3_g(state,  2,  7,  8, 13, m[s[12]], m[s[13]]);
        b3_g(state,  3,  4,  9, 14, m[s[14]], m[s[15]]);
    }
}

/* Compress and fold the state into a new 8-word chaining value. */
static void b3_compress_in_place(uint32_t cv[8],
                                 const uint8_t block[B3_BLOCK_LEN],
                                 uint32_t block_len, uint64_t counter,
                                 uint32_t flags) {
    uint32_t state[16];
    b3_compress_pre(state, cv, block, block_len, counter, flags);
    for (unsigned i = 0; i < 8; ++i) cv[i] = state[i] ^ state[i + 8u];
}

/* Wide compression. The first 32 bytes are the normal root output. */
static void b3_compress_xof(const uint32_t cv[8],
                            const uint8_t block[B3_BLOCK_LEN],
                            uint32_t block_len, uint64_t counter,
                            uint32_t flags, uint8_t out[64]) {
    uint32_t state[16];
    b3_compress_pre(state, cv, block, block_len, counter, flags);
    for (unsigned i = 0; i < 8; ++i) {
        b3_store32(out + 4u * i, state[i] ^ state[i + 8u]);
        b3_store32(out + 32u + 4u * i, state[8u + i] ^ cv[i]);
    }
}

/* One compression output, kept as (input_cv, block, len, counter, flags) so
 * the same object can yield a chaining value or a root digest. */
typedef struct B3Output {
    uint32_t input_cv[8];
    uint8_t block[B3_BLOCK_LEN];
    uint32_t block_len;
    uint64_t counter;
    uint32_t flags;
} B3Output;

static void b3_output_cv(const B3Output* o, uint8_t cv[B3_OUT_LEN]) {
    uint32_t words[8];
    for (unsigned i = 0; i < 8; ++i) words[i] = o->input_cv[i];
    b3_compress_in_place(words, o->block, o->block_len, o->counter, o->flags);
    for (unsigned i = 0; i < 8; ++i) b3_store32(cv + 4u * i, words[i]);
}

static void b3_output_root(const B3Output* o, uint8_t out[B3_OUT_LEN]) {
    uint8_t wide[64];
    b3_compress_xof(o->input_cv, o->block, o->block_len, o->counter,
                    o->flags | B3_ROOT, wide);
    for (unsigned i = 0; i < B3_OUT_LEN; ++i) out[i] = wide[i];
}

/* Build the final-block output for one chunk of at most 1024 bytes. The
 * first block carries CHUNK_START; the final block carries CHUNK_END. A
 * one-block chunk carries both flags, as the spec requires. */
static B3Output b3_chunk_output(const uint32_t key[8], const uint8_t* input,
                                size_t len, uint64_t counter, uint32_t flags) {
    B3Output o;
    for (unsigned i = 0; i < 8; ++i) o.input_cv[i] = key[i];
    memset(o.block, 0, B3_BLOCK_LEN);
    o.counter = counter;
    if (len > B3_BLOCK_LEN) {
        size_t full = (len - 1u) / B3_BLOCK_LEN;
        for (size_t i = 0; i < full; ++i) {
            uint32_t f = flags | (i == 0u ? B3_CHUNK_START : 0u);
            b3_compress_in_place(o.input_cv, input + i * B3_BLOCK_LEN,
                                 B3_BLOCK_LEN, counter, f);
        }
        size_t last_off = full * B3_BLOCK_LEN;
        size_t last_len = len - last_off;
        memcpy(o.block, input + last_off, last_len);
        o.block_len = (uint32_t)last_len;
        o.flags = flags | B3_CHUNK_END;
    } else {
        if (len > 0u) memcpy(o.block, input, len);
        o.block_len = (uint32_t)len;
        o.flags = flags | B3_CHUNK_START | B3_CHUNK_END;
    }
    return o;
}

static size_t b3_largest_pow2_le(size_t x) {
    size_t p = 1u;
    while ((p << 1) <= x && (p << 1) > p) p <<= 1;
    return p;
}

static void b3_parent_cv(const uint8_t left[B3_OUT_LEN],
                         const uint8_t right[B3_OUT_LEN], uint32_t flags,
                         uint8_t out[B3_OUT_LEN]) {
    uint8_t block[B3_BLOCK_LEN];
    memcpy(block, left, B3_OUT_LEN);
    memcpy(block + B3_OUT_LEN, right, B3_OUT_LEN);
    uint32_t cv[8];
    for (unsigned i = 0; i < 8; ++i) cv[i] = B3_IV[i];
    b3_compress_in_place(cv, block, B3_BLOCK_LEN, 0u, flags | B3_PARENT);
    for (unsigned i = 0; i < 8; ++i) b3_store32(out + 4u * i, cv[i]);
}

/* Hash a subtree and return its chaining value. Never applies ROOT. */
static void b3_hash_subtree(const uint8_t* input, size_t len,
                            const uint32_t key[8], uint64_t counter,
                            uint32_t flags, uint8_t out[B3_OUT_LEN]) {
    if (len <= B3_CHUNK_LEN) {
        B3Output o = b3_chunk_output(key, input, len, counter, flags);
        b3_output_cv(&o, out);
        return;
    }
    size_t chunks = (len - 1u) / B3_CHUNK_LEN + 1u;
    size_t left_chunks = b3_largest_pow2_le(chunks - 1u);
    size_t left_len = left_chunks * B3_CHUNK_LEN;
    uint8_t left[B3_OUT_LEN], right[B3_OUT_LEN];
    b3_hash_subtree(input, left_len, key, counter, flags, left);
    b3_hash_subtree(input + left_len, len - left_len, key,
                    counter + (uint64_t)left_chunks, flags, right);
    b3_parent_cv(left, right, flags, out);
}

void sheaf_blake3(const uint8_t* data, size_t len, uint8_t out[B3_OUT_LEN]) {
    if (len <= B3_CHUNK_LEN) {
        B3Output o = b3_chunk_output(B3_IV, data, len, 0u, 0u);
        b3_output_root(&o, out);
        return;
    }
    size_t chunks = (len - 1u) / B3_CHUNK_LEN + 1u;
    size_t left_chunks = b3_largest_pow2_le(chunks - 1u);
    size_t left_len = left_chunks * B3_CHUNK_LEN;
    uint8_t left[B3_OUT_LEN], right[B3_OUT_LEN];
    b3_hash_subtree(data, left_len, B3_IV, 0u, 0u, left);
    b3_hash_subtree(data + left_len, len - left_len, B3_IV,
                    (uint64_t)left_chunks, 0u, right);
    uint8_t block[B3_BLOCK_LEN];
    memcpy(block, left, B3_OUT_LEN);
    memcpy(block + B3_OUT_LEN, right, B3_OUT_LEN);
    uint32_t cv[8];
    for (unsigned i = 0; i < 8; ++i) cv[i] = B3_IV[i];
    uint8_t wide[64];
    b3_compress_xof(cv, block, B3_BLOCK_LEN, 0u, B3_PARENT | B3_ROOT, wide);
    for (unsigned i = 0; i < B3_OUT_LEN; ++i) out[i] = wide[i];
}

/* ---- BLAKE3 framed records ----------------------------------------------- */

#define SHEAF_B3_DIGEST_OFFSET 24u
#define SHEAF_B3_CRC_OFFSET 56u

void sheaf_frame_blake3_encode(uint8_t out[SHEAF_BLAKE3_RECORD_HEADER_BYTES],
                               uint64_t seq, uint64_t epoch,
                               const uint8_t* payload, uint32_t payload_len) {
    memset(out, 0, SHEAF_BLAKE3_RECORD_HEADER_BYTES);
    put_u32(out, SHEAF_REC_MAGIC);
    put_u64(out + 4, seq);
    put_u64(out + 12, epoch);
    put_u32(out + 20, payload_len);
    sheaf_blake3(payload, payload_len, out + SHEAF_B3_DIGEST_OFFSET);
    /* CRC32C remains the fast check on the header bytes. */
    put_u32(out + SHEAF_B3_CRC_OFFSET,
            sheaf_crc32c(out, SHEAF_B3_CRC_OFFSET));
}

int sheaf_frame_blake3_decode(const uint8_t in[SHEAF_BLAKE3_RECORD_HEADER_BYTES],
                              uint64_t* seq, uint64_t* epoch,
                              const uint8_t* payload, uint32_t payload_len) {
    if (get_u32(in) != SHEAF_REC_MAGIC) return SHEAF_WAL_ECORRUPT;
    if (get_u32(in + SHEAF_B3_CRC_OFFSET) !=
        sheaf_crc32c(in, SHEAF_B3_CRC_OFFSET)) return SHEAF_WAL_ECORRUPT;
    if (get_u32(in + 20) != payload_len) return SHEAF_WAL_ECORRUPT;
    uint8_t digest[SHEAF_BLAKE3_DIGEST_BYTES];
    sheaf_blake3(payload, payload_len, digest);
    if (memcmp(in + SHEAF_B3_DIGEST_OFFSET, digest,
               SHEAF_BLAKE3_DIGEST_BYTES) != 0) return SHEAF_WAL_ECORRUPT;
    if (seq != NULL) *seq = get_u64(in + 4);
    if (epoch != NULL) *epoch = get_u64(in + 12);
    return SHEAF_WAL_OK;
}
