// Timestamp: 2026-11-22_11-53-08 +05:30
// rle.cpp - C++23 run-length codec for u64 blocks (C-03).
//
// The format matches crates/sheaf-codec/src/rle.rs byte for byte:
//   tag u8 | count u32 | runs u32 | (run_len u32, value u64)*
// The Rust tier keeps the selector and the trait. This tier owns the encode
// and decode loops. Output must fit in the caller buffer, or the call fails.

#include "internal.hpp"

#include <cstdint>
#include <cstring>

extern "C" int sheaf_rle_encoded_bytes_u64(uint32_t count, uint32_t runs) {
    if (runs == 0) return 9;
    return 9 + static_cast<int>(runs) * 12;
}

extern "C" int sheaf_rle_encode_u64(const uint64_t* values, uint32_t count,
                                    uint8_t* out, uint32_t capacity,
                                    uint32_t* out_len) {
    if (!values || !out || !out_len || count == 0) return -1;
    uint32_t runs = 1;
    for (uint32_t i = 1; i < count; ++i) {
        if (values[i] != values[i - 1]) ++runs;
    }
    const int needed = sheaf_rle_encoded_bytes_u64(count, runs);
    if (needed < 0 || static_cast<uint32_t>(needed) > capacity) return -2;

    out[0] = 0; // TAG_U64
    std::memcpy(out + 1, &count, sizeof(uint32_t));
    std::memcpy(out + 5, &runs, sizeof(uint32_t));
    uint32_t at = 9;
    uint32_t start = 0;
    for (uint32_t i = 1; i <= count; ++i) {
        if (i == count || values[i] != values[start]) {
            const uint32_t length = i - start;
            std::memcpy(out + at, &length, sizeof(uint32_t));
            std::memcpy(out + at + 4, &values[start], sizeof(uint64_t));
            at += 12;
            start = i;
        }
    }
    *out_len = at;
    return 0;
}

extern "C" int sheaf_rle_decode_u64(const uint8_t* bytes, uint32_t len,
                                    uint64_t* out, uint32_t capacity,
                                    uint32_t* out_len) {
    if (!bytes || !out || !out_len || len < 9 || bytes[0] != 0) return -1;
    uint32_t count = 0;
    uint32_t runs = 0;
    std::memcpy(&count, bytes + 1, sizeof(uint32_t));
    std::memcpy(&runs, bytes + 5, sizeof(uint32_t));
    if (count == 0 || runs == 0 || count > capacity) return -1;
    uint32_t at = 9;
    uint32_t written = 0;
    for (uint32_t r = 0; r < runs; ++r) {
        if (at + 12 > len) return -1;
        uint32_t run_len = 0;
        uint64_t value = 0;
        std::memcpy(&run_len, bytes + at, sizeof(uint32_t));
        std::memcpy(&value, bytes + at + 4, sizeof(uint64_t));
        at += 12;
        if (run_len == 0 || run_len > count - written) return -1;
        for (uint32_t i = 0; i < run_len; ++i) out[written++] = value;
    }
    if (written != count) return -1;
    *out_len = written;
    return 0;
}
