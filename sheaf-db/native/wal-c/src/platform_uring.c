/* Linux io_uring write backend for the write-ahead log (WAL).
 *
 * Selected at build time with -DSHEAF_WAL_URING. This file owns the
 * sheaf_platform_write symbol on Linux in that configuration, and
 * platform_posix.c compiles its write out. All other operations stay
 * in platform_posix.c because they are off the hot append path.
 *
 * The ring uses IORING_SETUP_SQPOLL: a kernel thread polls the
 * submission queue, so a steady append stream submits without a
 * syscall per record. When the poller sleeps, the NEED_WAKEUP flag
 * tells us to enter the kernel once. Durability is unchanged: writes
 * land in the page cache and commit_epoch still calls fsync through
 * the POSIX backend.
 *
 * The WAL contract is single-writer, so the global ring needs no
 * lock. The ring lives for the process lifetime.
 *
 * If ring setup fails (old kernel, restricted sandbox), the write
 * falls back to a plain pwrite loop. The log stays correct, and the
 * only loss is the syscall-free submission path.
 */
#if defined(__linux__) && defined(SHEAF_WAL_URING)

#define _GNU_SOURCE

#include "wal_internal.h"

#include <errno.h>
#include <linux/io_uring.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>

#define SHEAF_URING_ENTRIES 64u
#define SHEAF_URING_IDLE_MS 1000u
#define SHEAF_URING_IO_CHUNK 0x40000000u

/* One process-wide ring. The single-writer WAL contract makes the
 * lazy init race-free without an atomic: the first append creates
 * the ring before any concurrent append can exist. */
typedef struct {
    int ring_fd;
    int ready;             /* 1 = usable, -1 = setup failed, 0 = new */
    uint32_t *sq_head;
    uint32_t *sq_tail;
    uint32_t *sq_flags;
    uint32_t *sq_array;
    struct io_uring_sqe *sqes;
    uint32_t *cq_head;
    uint32_t *cq_tail;
    struct io_uring_cqe *cqes;
    uint32_t sq_mask;
    uint32_t cq_mask;
} SheafRing;

static SheafRing g_ring;

static int ring_setup(void) {
    struct io_uring_params params;
    memset(&params, 0, sizeof(params));
    params.flags = IORING_SETUP_SQPOLL;
    params.sq_thread_idle = SHEAF_URING_IDLE_MS;

    int fd = (int)syscall(__NR_io_uring_setup, SHEAF_URING_ENTRIES, &params);
    if (fd < 0) {
        return -1;
    }

    size_t sq_ring_sz = params.sq_off.array +
                        params.sq_entries * sizeof(uint32_t);
    size_t cq_ring_sz = params.cq_off.cqes +
                        params.cq_entries * sizeof(struct io_uring_cqe);
    size_t sqe_sz = params.sq_entries * sizeof(struct io_uring_sqe);

    void *sq_ptr = mmap(NULL, sq_ring_sz, PROT_READ | PROT_WRITE,
                        MAP_SHARED | MAP_POPULATE, fd, IORING_OFF_SQ_RING);
    void *cq_ptr = mmap(NULL, cq_ring_sz, PROT_READ | PROT_WRITE,
                        MAP_SHARED | MAP_POPULATE, fd, IORING_OFF_CQ_RING);
    void *sqe_ptr = mmap(NULL, sqe_sz, PROT_READ | PROT_WRITE,
                         MAP_SHARED | MAP_POPULATE, fd, IORING_OFF_SQES);
    if (sq_ptr == MAP_FAILED || cq_ptr == MAP_FAILED || sqe_ptr == MAP_FAILED) {
        /* Unmap whatever succeeded, then report failure. The caller
         * falls back to pwrite, so a partial mapping is only a leak
         * if we keep it. */
        if (sq_ptr != MAP_FAILED) (void)munmap(sq_ptr, sq_ring_sz);
        if (cq_ptr != MAP_FAILED) (void)munmap(cq_ptr, cq_ring_sz);
        if (sqe_ptr != MAP_FAILED) (void)munmap(sqe_ptr, sqe_sz);
        (void)close(fd);
        return -1;
    }

    g_ring.ring_fd = fd;
    g_ring.sq_head = (uint32_t *)((uint8_t *)sq_ptr + params.sq_off.head);
    g_ring.sq_tail = (uint32_t *)((uint8_t *)sq_ptr + params.sq_off.tail);
    g_ring.sq_flags = (uint32_t *)((uint8_t *)sq_ptr + params.sq_off.flags);
    g_ring.sq_array = (uint32_t *)((uint8_t *)sq_ptr + params.sq_off.array);
    g_ring.sqes = (struct io_uring_sqe *)sqe_ptr;
    g_ring.cq_head = (uint32_t *)((uint8_t *)cq_ptr + params.cq_off.head);
    g_ring.cq_tail = (uint32_t *)((uint8_t *)cq_ptr + params.cq_off.tail);
    g_ring.cqes = (struct io_uring_cqe *)((uint8_t *)cq_ptr + params.cq_off.cqes);
    g_ring.sq_mask = params.sq_entries - 1;
    g_ring.cq_mask = params.cq_entries - 1;
    return 0;
}

/* Submit one pwrite and wait for its completion. Returns the byte
 * count, or a negative errno-style value on failure. */
static long ring_pwrite(int fd, const void *data, size_t len, uint64_t offset) {
    uint32_t tail = *g_ring.sq_tail;
    uint32_t head = __atomic_load_n(g_ring.sq_head, __ATOMIC_ACQUIRE);
    if (tail - head >= SHEAF_URING_ENTRIES) {
        return -EAGAIN; /* Queue full. The caller drains via retry. */
    }
    uint32_t idx = tail & g_ring.sq_mask;
    struct io_uring_sqe *sqe = &g_ring.sqes[idx];
    memset(sqe, 0, sizeof(*sqe));
    sqe->opcode = IORING_OP_WRITE;
    sqe->fd = fd;
    sqe->addr = (uint64_t)(uintptr_t)data;
    sqe->len = (uint32_t)len;
    sqe->off = offset;
    sqe->user_data = tail; /* Identifies this completion. */
    g_ring.sq_array[idx] = idx;
    __atomic_store_n(g_ring.sq_tail, tail + 1, __ATOMIC_RELEASE);

    /* SQPOLL normally sees the new tail without a syscall. Enter the
     * kernel only when the poller flagged itself asleep. */
    if (__atomic_load_n(g_ring.sq_flags, __ATOMIC_ACQUIRE) &
        IORING_SQ_NEED_WAKEUP) {
        long rc = syscall(__NR_io_uring_enter, g_ring.ring_fd,
                          1, 1, IORING_ENTER_SQ_WAKEUP, NULL, 0);
        if (rc < 0) {
            return -errno;
        }
    }

    /* Wait for our completion. Single-writer means the queue holds
     * only our entries, so a bounded spin then a blocking wait is
     * sufficient and keeps the code small. */
    for (unsigned spin = 0; spin < 4096u; ++spin) {
        uint32_t cq_head = __atomic_load_n(g_ring.cq_head, __ATOMIC_ACQUIRE);
        uint32_t cq_tail = __atomic_load_n(g_ring.cq_tail, __ATOMIC_ACQUIRE);
        if (cq_head != cq_tail) {
            struct io_uring_cqe *cqe = &g_ring.cqes[cq_head & g_ring.cq_mask];
            long res = cqe->res;
            __atomic_store_n(g_ring.cq_head, cq_head + 1, __ATOMIC_RELEASE);
            return res;
        }
    }
    long rc = syscall(__NR_io_uring_enter, g_ring.ring_fd,
                      0, 1, IORING_ENTER_GETEVENTS, NULL, 0);
    if (rc < 0) {
        return -errno;
    }
    uint32_t cq_head = *g_ring.cq_head;
    struct io_uring_cqe *cqe = &g_ring.cqes[cq_head & g_ring.cq_mask];
    long res = cqe->res;
    __atomic_store_n(g_ring.cq_head, cq_head + 1, __ATOMIC_RELEASE);
    return res;
}

/* pwrite fallback used when ring setup failed. Mirrors the POSIX
 * backend so behavior is identical on restricted kernels. */
static int fallback_pwrite(int fd, uint64_t offset,
                           const void *data, size_t len) {
    const uint8_t *cursor = (const uint8_t *)data;
    size_t done = 0;
    while (done < len) {
        size_t chunk = len - done;
        if (chunk > SHEAF_URING_IO_CHUNK) chunk = SHEAF_URING_IO_CHUNK;
        ssize_t n = pwrite(fd, cursor + done, chunk, (off_t)(offset + done));
        if (n < 0) {
            if (errno == EINTR) continue;
            return SHEAF_WAL_EIO;
        }
        if (n == 0) return SHEAF_WAL_EIO;
        done += (size_t)n;
    }
    return SHEAF_WAL_OK;
}

int sheaf_platform_write(SheafFile file, uint64_t offset,
                         const void *data, size_t len) {
    int fd = (int)(intptr_t)file;
    if (g_ring.ready == 0) {
        g_ring.ready = (ring_setup() == 0) ? 1 : -1;
    }
    if (g_ring.ready < 0) {
        return fallback_pwrite(fd, offset, data, len);
    }

    const uint8_t *cursor = (const uint8_t *)data;
    size_t done = 0;
    while (done < len) {
        size_t chunk = len - done;
        if (chunk > SHEAF_URING_IO_CHUNK) chunk = SHEAF_URING_IO_CHUNK;
        long n = ring_pwrite(fd, cursor + done, chunk, offset + done);
        if (n == -EINTR || n == -EAGAIN) {
            continue; /* Retry transient ring states. */
        }
        if (n < 0) {
            return SHEAF_WAL_EIO;
        }
        if (n == 0) {
            return SHEAF_WAL_EIO;
        }
        done += (size_t)n;
    }
    return SHEAF_WAL_OK;
}

#endif /* defined(__linux__) && defined(SHEAF_WAL_URING) */