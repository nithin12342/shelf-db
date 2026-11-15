/*
 * sheaf_wal_regbuf.h - C ABI for pre-registered io_uring buffers (Pillar 3).
 *
 * Purpose. A durable WAL write path pays one cost per record when the kernel
 * maps and pins the payload on every submit. io_uring removes that cost when
 * the buffers are registered once: the kernel pins them, and each submit
 * carries only a buffer index.
 *
 * Lifecycle:
 *
 *   sheaf_regbuf_pool_create   allocate and align the pool
 *   sheaf_regbuf_register      one io_uring_register_buffers call per pool
 *   sheaf_regbuf_write_fixed   IORING_OP_WRITE_FIXED with the buffer index
 *   sheaf_regbuf_unregister    release the pin
 *   sheaf_regbuf_pool_destroy  free the pool
 *
 * Return convention. Every function returns 0 on success. A negative value is
 * a library error code from this header. A positive value is the raw errno of
 * the failed system call, unchanged, so the caller can report the exact cause.
 *
 * Platform. The real path needs Linux and a kernel that offers io_uring. On
 * every other platform sheaf_regbuf_capability returns SHEAF_REGBUF_CAP_NONE
 * and sheaf_regbuf_capability_reason returns a sentence that names the cause.
 * A call that needs the capability then returns SHEAF_REGBUF_ECAP. The tier
 * never reports a fake success.
 *
 * Alignment. buffer_bytes must be a multiple of SHEAF_REGBUF_ALIGNMENT. The
 * pool then allocates one block and places buffer i at base + i*buffer_bytes,
 * so every buffer starts on a 64-byte boundary.
 */
#ifndef SHEAF_WAL_REGBUF_H
#define SHEAF_WAL_REGBUF_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Library error codes. Kept distinct from any errno value. */
#define SHEAF_REGBUF_OK        0
#define SHEAF_REGBUF_EINVAL  (-1)  /* bad pool, count, size, or length  */
#define SHEAF_REGBUF_EIO     (-2)  /* platform call failed with no errno */
#define SHEAF_REGBUF_ECAP    (-4)  /* io_uring is not available here     */
#define SHEAF_REGBUF_EALREADY (-5) /* the pool is already registered     */
#define SHEAF_REGBUF_EINDEX  (-6) /* the buffer index is outside the pool */

/* Every buffer starts on a boundary this wide. */
#define SHEAF_REGBUF_ALIGNMENT 64u

/* The largest pool that one registration accepts. */
#define SHEAF_REGBUF_MAX_BUFFERS 64u

/* Values for sheaf_regbuf_capability. */
#define SHEAF_REGBUF_CAP_NONE     0u
#define SHEAF_REGBUF_CAP_IO_URING 1u

typedef struct SheafRegBufPool SheafRegBufPool;

/*
 * The capability of this build and this kernel. SHEAF_REGBUF_CAP_IO_URING
 * means the real path is compiled in and io_uring_setup succeeded; the probe
 * does not allocate a pool.
 */
uint32_t sheaf_regbuf_capability(void);

/*
 * A sentence that names why the capability is absent, or an empty string when
 * the capability is present. The pointer stays valid for the process life.
 */
const char* sheaf_regbuf_capability_reason(void);

/*
 * Allocate a pool and open the target file. The pool is not registered yet.
 * Returns null when count is zero, count exceeds SHEAF_REGBUF_MAX_BUFFERS,
 * buffer_bytes is zero, or buffer_bytes is not a multiple of the alignment.
 * A null return is a bug at the call site, not a platform limit, so the
 * caller checks the argument first with sheaf_regbuf_check_args.
 */
SheafRegBufPool* sheaf_regbuf_pool_create(const char* path,
                                          uint32_t buffer_count,
                                          uint32_t buffer_bytes);

/* True when the arguments describe a pool that create accepts. */
int sheaf_regbuf_check_args(uint32_t buffer_count, uint32_t buffer_bytes);

/* Free the pool. An unregister is done first when the pool is registered. */
void sheaf_regbuf_pool_destroy(SheafRegBufPool* pool);

/*
 * Pin the pool in the kernel with one io_uring_register_buffers call.
 * Returns SHEAF_REGBUF_EALREADY on a second call, SHEAF_REGBUF_ECAP when the
 * platform has no io_uring, and a positive raw errno when the register call
 * fails.
 */
int sheaf_regbuf_register(SheafRegBufPool* pool);

/* Release the pin. A call on an unregistered pool succeeds and does nothing. */
int sheaf_regbuf_unregister(SheafRegBufPool* pool);

/* True when the pool is currently registered. */
int sheaf_regbuf_registered(const SheafRegBufPool* pool);

/*
 * Submit one IORING_OP_WRITE_FIXED at the start of buffer `index`, `len`
 * bytes into the file, then wait for the completion. On success out_written
 * holds the byte count that the kernel reported. A length above the buffer
 * size returns SHEAF_REGBUF_EINVAL. An index outside the pool returns
 * SHEAF_REGBUF_EINDEX.
 */
int sheaf_regbuf_write_fixed(SheafRegBufPool* pool, uint32_t index,
                             uint32_t len, uint32_t* out_written);

/* Flush the target file. Needed before a durability claim. */
int sheaf_regbuf_fsync(SheafRegBufPool* pool);

/* The writable bytes of buffer `index`, or null when the index is bad. */
uint8_t* sheaf_regbuf_buffer(SheafRegBufPool* pool, uint32_t index);

/* The number of buffers in the pool. */
uint32_t sheaf_regbuf_count(const SheafRegBufPool* pool);

/* The bytes of one buffer. */
uint32_t sheaf_regbuf_buffer_bytes(const SheafRegBufPool* pool);

/* The last raw errno that a failed system call reported, or zero. */
int32_t sheaf_regbuf_last_errno(const SheafRegBufPool* pool);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SHEAF_WAL_REGBUF_H */
