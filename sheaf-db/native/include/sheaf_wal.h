/*
 * sheaf_wal.h - C ABI contract for the Tier 2b write-ahead log (Pillar 3).
 *
 * Durability model:
 *   - Mutations append framed records to an append-only log slab.
 *   - sheaf_wal_commit_epoch is the durability boundary: it flushes the
 *     log, then writes the INACTIVE epoch header slot (A/B alternation)
 *     and flushes the header file. A power cut during the slot write
 *     leaves the other slot valid, so the previous epoch survives.
 *   - Recovery picks the valid slot with the highest epoch, scans records
 *     up to its valid_end, and truncates any torn tail.
 *
 * Framing uses CRC32C for fast sector integrity. A BLAKE3 frame mode also
 * puts a BLAKE3-256 checksum in every framed record (spec Pillar 3, line
 * 241). BLAKE3 content addressing for whole sections stays in sheaf-vcs.
 *
 * Platform backends (one is compiled per build):
 *   - Windows: WriteFile + FlushFileBuffers, FILE_FLAG_WRITE_THROUGH.
 *   - POSIX:   pwrite + fsync.
 *   - Linux:   io_uring SQPOLL when SHEAF_WAL_URING is defined. The kernel
 *              polling thread submits writes with zero enter syscalls in
 *              steady state (spec: zero-syscall logging).
 */
#ifndef SHEAF_WAL_H
#define SHEAF_WAL_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define SHEAF_WAL_OK        0
#define SHEAF_WAL_EINVAL  (-1)  /* bad argument or non-monotonic epoch */
#define SHEAF_WAL_EIO     (-2)  /* platform read/write failure         */
#define SHEAF_WAL_ECORRUPT (-3) /* both header slots invalid on a nonempty log */

/* Sector size of one epoch header slot. Two slots occupy one 8 KB file. */
#define SHEAF_WAL_SLOT_BYTES 4096u

/* Maximum payload bytes per record. Stays inside common NVMe AWUPF
 * budgets (4 KB .. 64 KB) so a record append is power-fail atomic on
 * drives that advertise AWUPF >= this size. */
#define SHEAF_WAL_MAX_PAYLOAD 65536u

/* BLAKE3 frame mode constants. CRC32C stays as the fast sector and header
 * check. The BLAKE3-256 digest covers each record payload. */
#define SHEAF_BLAKE3_DIGEST_BYTES 32u
#define SHEAF_BLAKE3_RECORD_HEADER_BYTES 60u

typedef struct SheafWal SheafWal;

/* Recovery report filled by sheaf_wal_recover (also run inside open). */
typedef struct SheafWalStats {
    uint64_t epoch;              /* epoch of the winning header slot   */
    uint64_t last_seq;           /* highest valid record sequence      */
    uint64_t valid_end;          /* log bytes covered by the commit    */
    uint64_t records_recovered;  /* records validated during the scan  */
    uint64_t truncated_bytes;    /* torn or uncommitted tail removed   */
} SheafWalStats;

/*
 * Open (or create) the log and header files, then run recovery.
 * Returns null on failure. Paths are borrowed for the call only.
 */
SheafWal* sheaf_wal_open(const char* log_path, const char* header_path);

/*
 * Append one framed record. The record becomes durable only after the
 * next sheaf_wal_commit_epoch. Returns 0 or a negative error code.
 */
int sheaf_wal_append(SheafWal* wal, uint64_t epoch,
                     const uint8_t* payload, uint32_t len);

/*
 * Durability boundary: flush the log, then flip the epoch header to the
 * inactive slot with new_epoch. new_epoch must exceed the current epoch.
 */
int sheaf_wal_commit_epoch(SheafWal* wal, uint64_t new_epoch);

/* Force the platform barrier without flipping the epoch (group commit). */
int sheaf_wal_flush(SheafWal* wal);

/* Re-scan and repair. Safe to call repeatedly. Fills out when non-null. */
int sheaf_wal_recover(SheafWal* wal, SheafWalStats* out);

/* Current committed epoch. */
uint64_t sheaf_wal_epoch(const SheafWal* wal);

/* Flush and release. Null is ignored. */
void sheaf_wal_close(SheafWal* wal);

/* ---- BLAKE3 frame mode (frame_blake3.c) ---------------------------------- */

/* Compute the BLAKE3-256 digest of data[0..len) into out[32]. */
void sheaf_blake3(const uint8_t* data, size_t len,
                  uint8_t out[SHEAF_BLAKE3_DIGEST_BYTES]);

/* Serialize a BLAKE3 framed record header. The 32-byte digest covers
 * payload[0..len). The trailing CRC32C covers bytes [0..56) of the header. */
void sheaf_frame_blake3_encode(uint8_t out[SHEAF_BLAKE3_RECORD_HEADER_BYTES],
                               uint64_t seq, uint64_t epoch,
                               const uint8_t* payload, uint32_t len);

/* Validate a BLAKE3 framed record against a payload. The check covers the
 * magic, the header CRC32C, the payload length, and the BLAKE3 digest.
 * Returns SHEAF_WAL_OK or SHEAF_WAL_ECORRUPT. seq and epoch may be null. */
int sheaf_frame_blake3_decode(const uint8_t in[SHEAF_BLAKE3_RECORD_HEADER_BYTES],
                              uint64_t* seq, uint64_t* epoch,
                              const uint8_t* payload, uint32_t len);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SHEAF_WAL_H */