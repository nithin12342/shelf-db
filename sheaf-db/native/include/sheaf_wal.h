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
 * Framing uses CRC32C for fast sector integrity. BLAKE3 content addressing
 * lives in the Rust commit layer (sheaf-vcs), which hashes whole sections.
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

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SHEAF_WAL_H */