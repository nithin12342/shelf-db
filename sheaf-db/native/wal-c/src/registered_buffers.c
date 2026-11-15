/*
 * registered_buffers.c - pre-registered io_uring write buffers (MF-08).
 *
 * One pool owns one 64-byte aligned block and one registration. The kernel
 * pins the block once, and each fixed write carries only the buffer index.
 * That removes the per-record map and pin cost of the normal write path.
 *
 * Platform split:
 *   Linux with -DSHEAF_WAL_URING  real io_uring_setup, io_uring_register,
 *                                 and IORING_OP_WRITE_FIXED.
 *   Every other target            the pool still allocates and aligns, but
 *                                 register and write_fixed return
 *                                 SHEAF_REGBUF_ECAP and
 *                                 sheaf_regbuf_capability_reason names the
 *                                 cause. The tier never reports a fake
 *                                 success.
 *
 * Return convention: 0 is success, a negative value is a library error from
 * sheaf_wal_regbuf.h, and a positive value is the raw errno of the failed
 * system call.
 */

#if !defined(_WIN32) && !defined(_POSIX_C_SOURCE)
#define _POSIX_C_SOURCE 200809L
#endif

#include "../include/sheaf_wal_regbuf.h"

#include <errno.h>
#include <stdlib.h>
#include <string.h>

#if defined(_WIN32)
#  include <fcntl.h>
#  include <io.h>
#  include <malloc.h>
#  include <sys/stat.h>
#  include <sys/types.h>
#else
#  include <fcntl.h>
#  include <sys/stat.h>
#  include <sys/types.h>
#  include <unistd.h>
#endif

#if defined(__linux__) && defined(SHEAF_WAL_URING)
#define SHEAF_REGBUF_HAS_URING 1
#else
#define SHEAF_REGBUF_HAS_URING 0
#endif

#if SHEAF_REGBUF_HAS_URING
#include <linux/io_uring.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <sys/uio.h>

/* One submission and completion ring. The kernel owns the memory behind it. */
struct sheaf_uring {
    int ring_fd;
    void* sq_ring;
    size_t sq_ring_sz;
    void* cq_ring;
    size_t cq_ring_sz;
    struct io_uring_sqe* sqes;
    size_t sqes_sz;
    unsigned* sq_head;
    unsigned* sq_tail;
    unsigned* sq_ring_mask;
    unsigned* sq_array;
    unsigned* cq_head;
    unsigned* cq_tail;
    unsigned* cq_ring_mask;
    struct io_uring_cqe* cqes;
};
#endif

/* One pool: the aligned block, the target file, and the registration state. */
struct SheafRegBufPool {
    void* block;
    uint8_t* base;
    uint32_t count;
    uint32_t buffer_bytes;
    int registered;
    int fd;
    int32_t last_errno;
#if SHEAF_REGBUF_HAS_URING
    struct sheaf_uring ring;
    int ring_ready;
#endif
};

/* Allocate `bytes` on a SHEAF_REGBUF_ALIGNMENT boundary. */
static void* aligned_block(size_t bytes) {
#if defined(_WIN32)
    return _aligned_malloc(bytes, SHEAF_REGBUF_ALIGNMENT);
#else
    void* p = NULL;
    if (posix_memalign(&p, SHEAF_REGBUF_ALIGNMENT, bytes) != 0) {
        return NULL;
    }
    return p;
#endif
}

static void aligned_block_free(void* p) {
#if defined(_WIN32)
    _aligned_free(p);
#else
    free(p);
#endif
}

/* Open the target file for writing, then make it exactly zero bytes long. */
static int open_target(const char* path) {
#if defined(_WIN32)
    return _open(path, _O_WRONLY | _O_CREAT | _O_TRUNC | _O_BINARY,
                 _S_IREAD | _S_IWRITE);
#else
    return open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
#endif
}

static void close_target(int fd) {
#if defined(_WIN32)
    _close(fd);
#else
    close(fd);
#endif
}

/* Force the target file to stable storage. */
static int sync_target(int fd) {
#if defined(_WIN32)
    return _commit(fd);
#else
    return fsync(fd);
#endif
}

#if SHEAF_REGBUF_HAS_URING

/* Build one ring. Returns 0, or -1 with *out_errno set to the raw errno. */
static int uring_init(struct sheaf_uring* r, unsigned entries, int32_t* out_errno) {
    struct io_uring_params p;
    memset(&p, 0, sizeof(p));
    int fd = (int)syscall(__NR_io_uring_setup, entries, &p);
    if (fd < 0) {
        *out_errno = errno;
        return -1;
    }
    memset(r, 0, sizeof(*r));
    r->ring_fd = fd;

    size_t sq_sz = (size_t)p.sq_off.array + sizeof(unsigned) * p.sq_entries;
    size_t cq_sz = (size_t)p.cq_off.cqes + sizeof(struct io_uring_cqe) * p.cq_entries;
    if (p.features & IORING_FEAT_SINGLE_MMAP) {
        /* One mapping serves both rings, so both sizes must agree. */
        if (cq_sz > sq_sz) {
            sq_sz = cq_sz;
        }
        cq_sz = sq_sz;
    }

    void* sq = mmap(NULL, sq_sz, PROT_READ | PROT_WRITE,
                    MAP_SHARED | MAP_POPULATE, fd, IORING_OFF_SQ_RING);
    if (sq == MAP_FAILED) {
        *out_errno = errno;
        close_target(fd);
        return -1;
    }
    void* cq = sq;
    if (!(p.features & IORING_FEAT_SINGLE_MMAP)) {
        cq = mmap(NULL, cq_sz, PROT_READ | PROT_WRITE,
                  MAP_SHARED | MAP_POPULATE, fd, IORING_OFF_CQ_RING);
        if (cq == MAP_FAILED) {
            *out_errno = errno;
            munmap(sq, sq_sz);
            close_target(fd);
            return -1;
        }
    }
    size_t sqes_sz = sizeof(struct io_uring_sqe) * p.sq_entries;
    void* sqes = mmap(NULL, sqes_sz, PROT_READ | PROT_WRITE,
                      MAP_SHARED | MAP_POPULATE, fd, IORING_OFF_SQES);
    if (sqes == MAP_FAILED) {
        *out_errno = errno;
        if (cq != sq) {
            munmap(cq, cq_sz);
        }
        munmap(sq, sq_sz);
        close_target(fd);
        return -1;
    }

    r->sq_ring = sq;
    r->sq_ring_sz = sq_sz;
    r->cq_ring = cq;
    r->cq_ring_sz = cq_sz;
    r->sqes = (struct io_uring_sqe*)sqes;
    r->sqes_sz = sqes_sz;

    uint8_t* sqb = (uint8_t*)sq;
    r->sq_head = (unsigned*)(void*)(sqb + p.sq_off.head);
    r->sq_tail = (unsigned*)(void*)(sqb + p.sq_off.tail);
    r->sq_ring_mask = (unsigned*)(void*)(sqb + p.sq_off.ring_mask);
    r->sq_array = (unsigned*)(void*)(sqb + p.sq_off.array);

    uint8_t* cqb = (uint8_t*)cq;
    r->cq_head = (unsigned*)(void*)(cqb + p.cq_off.head);
    r->cq_tail = (unsigned*)(void*)(cqb + p.cq_off.tail);
    r->cq_ring_mask = (unsigned*)(void*)(cqb + p.cq_off.ring_mask);
    r->cqes = (struct io_uring_cqe*)(void*)(cqb + p.cq_off.cqes);
    return 0;
}

static void uring_destroy(struct sheaf_uring* r) {
    if (r->sqes != NULL) {
        munmap(r->sqes, r->sqes_sz);
    }
    if (r->cq_ring != NULL && r->cq_ring != r->sq_ring) {
        munmap(r->cq_ring, r->cq_ring_sz);
    }
    if (r->sq_ring != NULL) {
        munmap(r->sq_ring, r->sq_ring_sz);
    }
    if (r->ring_fd > 0) {
        close_target(r->ring_fd);
    }
    memset(r, 0, sizeof(*r));
}

/* Submit one fixed write and wait for its completion. */
static int uring_write_fixed(struct sheaf_uring* r, int fd, const uint8_t* addr,
                             unsigned len, unsigned buf_index, uint64_t off,
                             int32_t* out_errno) {
    unsigned tail = *r->sq_tail;
    unsigned index = tail & *r->sq_ring_mask;
    struct io_uring_sqe* sqe = &r->sqes[index];
    memset(sqe, 0, sizeof(*sqe));
    sqe->opcode = IORING_OP_WRITE_FIXED;
    sqe->fd = fd;
    sqe->addr = (uint64_t)(uintptr_t)addr;
    sqe->len = len;
    sqe->off = off;
    sqe->buf_index = (uint16_t)buf_index;
    sqe->user_data = 0x5348454146ull;
    r->sq_array[index] = index;
    __atomic_store_n(r->sq_tail, tail + 1, __ATOMIC_RELEASE);

    long rc = syscall(__NR_io_uring_enter, r->ring_fd, 1, 1,
                      IORING_ENTER_GETEVENTS, NULL, 0);
    if (rc < 0) {
        *out_errno = errno;
        return -1;
    }

    for (;;) {
        unsigned head = __atomic_load_n(r->cq_head, __ATOMIC_ACQUIRE);
        if (head != *r->cq_tail) {
            struct io_uring_cqe* cqe = &r->cqes[head & *r->cq_ring_mask];
            int res = cqe->res;
            __atomic_store_n(r->cq_head, head + 1, __ATOMIC_RELEASE);
            if (res < 0) {
                *out_errno = -res;
                return -1;
            }
            return res;
        }
        rc = syscall(__NR_io_uring_enter, r->ring_fd, 0, 1,
                     IORING_ENTER_GETEVENTS, NULL, 0);
        if (rc < 0) {
            *out_errno = errno;
            return -1;
        }
    }
}

/* Open and close one ring. A closed ring is enough to prove the capability. */
static int probe_uring(void) {
    struct io_uring_params p;
    memset(&p, 0, sizeof(p));
    int fd = (int)syscall(__NR_io_uring_setup, 2, &p);
    if (fd < 0) {
        return -1;
    }
    close_target(fd);
    return 0;
}

uint32_t sheaf_regbuf_capability(void) {
    return probe_uring() == 0 ? SHEAF_REGBUF_CAP_IO_URING : SHEAF_REGBUF_CAP_NONE;
}

const char* sheaf_regbuf_capability_reason(void) {
    if (probe_uring() == 0) {
        return "";
    }
    return "the kernel refused io_uring_setup, so the registered-buffer write path is absent";
}

#else

uint32_t sheaf_regbuf_capability(void) {
    return SHEAF_REGBUF_CAP_NONE;
}

const char* sheaf_regbuf_capability_reason(void) {
#if defined(_WIN32)
    return "io_uring is a Linux interface, so this platform uses the normal blocking WAL write path";
#elif defined(__linux__)
    return "this build did not define SHEAF_WAL_URING, so the io_uring fixed-write path is absent";
#else
    return "this platform has no io_uring, so the WAL uses the normal blocking write path";
#endif
}

#endif /* SHEAF_REGBUF_HAS_URING */

int sheaf_regbuf_check_args(uint32_t buffer_count, uint32_t buffer_bytes) {
    if (buffer_count == 0 || buffer_count > SHEAF_REGBUF_MAX_BUFFERS) {
        return SHEAF_REGBUF_EINVAL;
    }
    if (buffer_bytes == 0) {
        return SHEAF_REGBUF_EINVAL;
    }
    if (buffer_bytes % SHEAF_REGBUF_ALIGNMENT != 0) {
        return SHEAF_REGBUF_EINVAL;
    }
    return SHEAF_REGBUF_OK;
}

SheafRegBufPool* sheaf_regbuf_pool_create(const char* path,
                                          uint32_t buffer_count,
                                          uint32_t buffer_bytes) {
    if (path == NULL || sheaf_regbuf_check_args(buffer_count, buffer_bytes) != SHEAF_REGBUF_OK) {
        return NULL;
    }
    size_t total = (size_t)buffer_count * (size_t)buffer_bytes;
    if (total / buffer_bytes != (size_t)buffer_count) {
        return NULL;
    }
    void* block = aligned_block(total);
    if (block == NULL) {
        return NULL;
    }
    memset(block, 0, total);

    int fd = open_target(path);
    if (fd < 0) {
        aligned_block_free(block);
        return NULL;
    }
    SheafRegBufPool* pool = (SheafRegBufPool*)calloc(1, sizeof(*pool));
    if (pool == NULL) {
        close_target(fd);
        aligned_block_free(block);
        return NULL;
    }
    pool->block = block;
    pool->base = (uint8_t*)block;
    pool->count = buffer_count;
    pool->buffer_bytes = buffer_bytes;
    pool->registered = 0;
    pool->fd = fd;
    pool->last_errno = 0;
    return pool;
}

void sheaf_regbuf_pool_destroy(SheafRegBufPool* pool) {
    if (pool == NULL) {
        return;
    }
    (void)sheaf_regbuf_unregister(pool);
#if SHEAF_REGBUF_HAS_URING
    if (pool->ring_ready) {
        uring_destroy(&pool->ring);
    }
#endif
    if (pool->fd >= 0) {
        close_target(pool->fd);
    }
    if (pool->block != NULL) {
        aligned_block_free(pool->block);
    }
    free(pool);
}

int sheaf_regbuf_register(SheafRegBufPool* pool) {
    if (pool == NULL) {
        return SHEAF_REGBUF_EINVAL;
    }
    if (pool->registered) {
        return SHEAF_REGBUF_EALREADY;
    }
#if SHEAF_REGBUF_HAS_URING
    if (sheaf_regbuf_capability() != SHEAF_REGBUF_CAP_IO_URING) {
        return SHEAF_REGBUF_ECAP;
    }
    if (!pool->ring_ready) {
        int32_t cause = 0;
        if (uring_init(&pool->ring, 8, &cause) != 0) {
            pool->last_errno = cause;
            return cause > 0 ? cause : SHEAF_REGBUF_EIO;
        }
        pool->ring_ready = 1;
    }
    struct iovec vec[SHEAF_REGBUF_MAX_BUFFERS];
    uint32_t i;
    for (i = 0; i < pool->count; i++) {
        vec[i].iov_base = pool->base + (size_t)i * pool->buffer_bytes;
        vec[i].iov_len = pool->buffer_bytes;
    }
    long rc = syscall(__NR_io_uring_register, pool->ring.ring_fd,
                      IORING_REGISTER_BUFFERS, vec, (unsigned)pool->count);
    if (rc < 0) {
        /* A registration failure reports the raw errno, unchanged. */
        pool->last_errno = errno;
        return errno > 0 ? errno : SHEAF_REGBUF_EIO;
    }
    pool->registered = 1;
    return SHEAF_REGBUF_OK;
#else
    (void)pool->last_errno;
    return SHEAF_REGBUF_ECAP;
#endif
}

int sheaf_regbuf_unregister(SheafRegBufPool* pool) {
    if (pool == NULL) {
        return SHEAF_REGBUF_EINVAL;
    }
#if SHEAF_REGBUF_HAS_URING
    if (pool->registered) {
        long rc = syscall(__NR_io_uring_register, pool->ring.ring_fd,
                          IORING_UNREGISTER_BUFFERS, NULL, 0);
        if (rc < 0) {
            pool->last_errno = errno;
            return errno > 0 ? errno : SHEAF_REGBUF_EIO;
        }
        pool->registered = 0;
    }
#else
    pool->registered = 0;
#endif
    return SHEAF_REGBUF_OK;
}

int sheaf_regbuf_registered(const SheafRegBufPool* pool) {
    return pool != NULL && pool->registered != 0;
}

int sheaf_regbuf_write_fixed(SheafRegBufPool* pool, uint32_t index,
                             uint32_t len, uint32_t* out_written) {
    if (pool == NULL || out_written == NULL) {
        return SHEAF_REGBUF_EINVAL;
    }
    if (index >= pool->count) {
        return SHEAF_REGBUF_EINDEX;
    }
    if (len == 0 || len > pool->buffer_bytes) {
        return SHEAF_REGBUF_EINVAL;
    }
#if SHEAF_REGBUF_HAS_URING
    if (!pool->registered) {
        return SHEAF_REGBUF_ECAP;
    }
    int32_t cause = 0;
    const uint8_t* addr = pool->base + (size_t)index * pool->buffer_bytes;
    /* Each buffer owns its own slot, so buffer i writes at offset i*size. */
    uint64_t off = (uint64_t)index * (uint64_t)pool->buffer_bytes;
    int rc = uring_write_fixed(&pool->ring, pool->fd, addr, len, index, off, &cause);
    if (rc < 0) {
        pool->last_errno = cause;
        return cause > 0 ? cause : SHEAF_REGBUF_EIO;
    }
    *out_written = (uint32_t)rc;
    return SHEAF_REGBUF_OK;
#else
    (void)index;
    (void)len;
    return SHEAF_REGBUF_ECAP;
#endif
}

int sheaf_regbuf_fsync(SheafRegBufPool* pool) {
    if (pool == NULL) {
        return SHEAF_REGBUF_EINVAL;
    }
    if (sync_target(pool->fd) != 0) {
        pool->last_errno = errno;
        return errno > 0 ? errno : SHEAF_REGBUF_EIO;
    }
    return SHEAF_REGBUF_OK;
}

uint8_t* sheaf_regbuf_buffer(SheafRegBufPool* pool, uint32_t index) {
    if (pool == NULL || index >= pool->count) {
        return NULL;
    }
    return pool->base + (size_t)index * pool->buffer_bytes;
}

uint32_t sheaf_regbuf_count(const SheafRegBufPool* pool) {
    return pool == NULL ? 0u : pool->count;
}

uint32_t sheaf_regbuf_buffer_bytes(const SheafRegBufPool* pool) {
    return pool == NULL ? 0u : pool->buffer_bytes;
}

int32_t sheaf_regbuf_last_errno(const SheafRegBufPool* pool) {
    return pool == NULL ? 0 : pool->last_errno;
}
