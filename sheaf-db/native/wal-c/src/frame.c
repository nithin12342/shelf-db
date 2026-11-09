/*
 * frame.c - self-verifying record framing for the WAL (spec Pillar 3).
 *
 * Every record carries magic, a monotonic sequence, the writer epoch, the
 * payload length, a payload CRC32C, and a header CRC32C. Recovery can
 * therefore distinguish three states at any byte offset: a valid record,
 * a torn tail (truncate), or bit rot inside the committed region (stop).
 *
 * CRC32C (Castagnoli, reflected polynomial 0x82F63B78) is computed with
 * the bitwise recurrence. It is fast enough for framing at log rates and
 * needs no tables. Hardware CRC instructions can replace this function
 * later without changing the wire format.
 */
#include "wal_internal.h"

/* ---- CRC32C ------------------------------------------------------------- */

uint32_t sheaf_crc32c(const uint8_t* data, size_t len) {
    uint32_t crc = 0xFFFFFFFFu;
    for (size_t i = 0; i < len; ++i) {
        crc ^= data[i];
        for (int bit = 0; bit < 8; ++bit) {
            /* Branchless step: mask is all-ones when the low bit is set. */
            const uint32_t mask = (uint32_t)0 - (crc & 1u);
            crc = (crc >> 1) ^ (0x82F63B78u & mask);
        }
    }
    return ~crc;
}

/* ---- record header ------------------------------------------------------ */

void sheaf_frame_encode(uint8_t out[32], const SheafRecHeader* h,
                        const uint8_t* payload) {
    put_u32(out, SHEAF_REC_MAGIC);
    put_u64(out + 4, h->seq);
    put_u64(out + 12, h->epoch);
    put_u32(out + 20, h->payload_len);
    put_u32(out + 24, sheaf_crc32c(payload, h->payload_len));
    /* The header CRC covers every preceding field, including both CRCs
     * except itself. A torn header write fails this check first. */
    put_u32(out + 28, sheaf_crc32c(out, 28));
}

int sheaf_frame_decode(const uint8_t in[32], SheafRecHeader* out,
                       uint32_t* payload_crc) {
    if (get_u32(in) != SHEAF_REC_MAGIC) return -1;
    if (get_u32(in + 28) != sheaf_crc32c(in, 28)) return -1;
    out->seq = get_u64(in + 4);
    out->epoch = get_u64(in + 12);
    out->payload_len = get_u32(in + 20);
    *payload_crc = get_u32(in + 24);
    return 0;
}