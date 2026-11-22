/*
 * frame_blake3_test.c - C11 unit test for the WAL BLAKE3 frame mode.
 *
 * Timestamp: 2026-11-22_12-48-26 +05:30
 * Remote: origin https://github.com/nithin12342/shelf-db.git
 * Branch: combined-stack-feature-improvement
 * Base commit: 3832065581f3ff4240f4d267c38bcde8fea1c24e
 *
 * This test uses only the public C ABI from native/include/sheaf_wal.h. It
 * calls the production functions in frame_blake3.c with real input. It has
 * no dependency on Rust or on the internal WAL headers.
 *
 * Build (from the repository root, with a C11 compiler on PATH):
 *   cc -std=c11 -Wall -Wextra -pedantic \
 *      -I native/include \
 *      native/wal-c/src/frame.c native/wal-c/src/frame_blake3.c \
 *      native/wal-c/tests/frame_blake3_test.c -o frame_blake3_test.exe
 */
#include "sheaf_wal.h"

#include <stdio.h>
#include <string.h>

static int g_failures = 0;

static void check(int condition, const char* name) {
    if (condition) {
        printf("PASS %s\n", name);
    } else {
        printf("FAIL %s\n", name);
        ++g_failures;
    }
}

/* Compare a 32-byte digest with a 64-character lowercase hex reference. */
static int digest_is(const uint8_t digest[SHEAF_BLAKE3_DIGEST_BYTES],
                     const char* hex) {
    static const char digits[] = "0123456789abcdef";
    char rendered[2 * SHEAF_BLAKE3_DIGEST_BYTES + 1];
    for (unsigned i = 0; i < SHEAF_BLAKE3_DIGEST_BYTES; ++i) {
        rendered[2u * i] = digits[digest[i] >> 4];
        rendered[2u * i + 1u] = digits[digest[i] & 0x0Fu];
    }
    rendered[2 * SHEAF_BLAKE3_DIGEST_BYTES] = '\0';
    return strcmp(rendered, hex) == 0;
}

/* Reference vectors. The empty and "abc" vectors are the published BLAKE3
 * test vectors. The three longer vectors were computed independently with
 * blake3 crate 1.8.7 over data[i] = i % 251. They exercise the multi-block
 * and multi-chunk tree paths. They are external references, not values
 * returned by the code under test. */
static void test_blake3_vectors(void) {
    uint8_t out[SHEAF_BLAKE3_DIGEST_BYTES];

    sheaf_blake3((const uint8_t*)"", 0u, out);
    check(digest_is(out,
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"),
        "blake3 empty vector");

    sheaf_blake3((const uint8_t*)"abc", 3u, out);
    check(digest_is(out,
        "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"),
        "blake3 abc vector");

    const size_t lengths[3] = { 1024u, 1025u, 4096u };
    const char* expected[3] = {
        "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7",
        "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444",
        "015094013f57a5277b59d8475c0501042c0b642e531b0a1c8f58d2163229e969"
    };
    uint8_t data[4096];
    for (unsigned i = 0; i < 4096u; ++i) data[i] = (uint8_t)(i % 251u);
    for (unsigned k = 0; k < 3u; ++k) {
        sheaf_blake3(data, lengths[k], out);
        check(digest_is(out, expected[k]),
              k == 0u ? "blake3 1024-byte full chunk vector" :
              k == 1u ? "blake3 1025-byte two-chunk vector" :
                        "blake3 4096-byte four-chunk vector");
    }
}

/* The C-05 gate: one changed payload byte fails the BLAKE3 record check. */
static void test_record_change_detection(void) {
    uint8_t header[SHEAF_BLAKE3_RECORD_HEADER_BYTES];
    uint8_t payload[64];
    uint8_t changed[64];
    uint64_t seq = 0u;
    uint64_t epoch = 0u;

    for (unsigned i = 0; i < 64u; ++i) payload[i] = (uint8_t)i;
    memcpy(changed, payload, sizeof payload);

    sheaf_frame_blake3_encode(header, 7u, 9u, payload, (uint32_t)sizeof payload);

    int rc = sheaf_frame_blake3_decode(header, &seq, &epoch,
                                       payload, (uint32_t)sizeof payload);
    check(rc == SHEAF_WAL_OK, "unchanged payload passes BLAKE3 record check");
    check(seq == 7u && epoch == 9u, "seq and epoch round trip");

    /* Change exactly one payload byte. Every header byte stays the same. */
    changed[13] ^= 0x01u;
    rc = sheaf_frame_blake3_decode(header, NULL, NULL,
                                   changed, (uint32_t)sizeof changed);
    check(rc == SHEAF_WAL_ECORRUPT,
          "one changed payload byte fails BLAKE3 record check");

    uint8_t d1[SHEAF_BLAKE3_DIGEST_BYTES];
    uint8_t d2[SHEAF_BLAKE3_DIGEST_BYTES];
    sheaf_blake3(payload, sizeof payload, d1);
    sheaf_blake3(changed, sizeof changed, d2);
    check(memcmp(d1, d2, SHEAF_BLAKE3_DIGEST_BYTES) != 0,
          "BLAKE3 digest differs for the changed payload");

    rc = sheaf_frame_blake3_decode(header, NULL, NULL,
                                   payload, (uint32_t)sizeof payload - 1u);
    check(rc == SHEAF_WAL_ECORRUPT, "payload length mismatch fails");
}

int main(void) {
    printf("C-05 BLAKE3 frame test (C11, public C ABI)\n");
    test_blake3_vectors();
    test_record_change_detection();
    printf("failures=%d\n", g_failures);
    return g_failures == 0 ? 0 : 1;
}
