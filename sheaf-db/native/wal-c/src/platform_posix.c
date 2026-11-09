/* POSIX backend for the write-ahead log (WAL) file interface.
 *
 * The WAL needs three durability guarantees:
 *   1. A write reaches the kernel in full, at the exact offset.
 *   2. A flush makes the bytes durable on stable storage.
 *   3. A truncate removes a torn tail after a crash.
 *
 * This file maps those guarantees onto pwrite(2), fsync(2), and
 * ftruncate(2). The io_uring backend (platform_uring.c) replaces the
 * write path on Linux when SHEAF_WAL_URING selects it. The two
 * backends share this file for open, read, flush, size, truncate,
 * and close, because those calls are not on the hot append path.
 */
#if !defined(_WIN32)

#define _POSIX_C_SOURCE 200809L

#include "wal_internal.h"

#include <errno.h>
#include <fcntl.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

/* Write chunk cap. A single pwrite of a very large buffer can return
 * a short count on some kernels. Looping in bounded chunks keeps the
 * retry logic simple and avoids implementation-specific limits. */
#define SHEAF_POSIX_IO_CHUNK 0x40000000u

int sheaf_platform_open(const char *path, SheafFile *out) {
    if (path == NULL || out == NULL) {
        return SHEAF_WAL_EINVAL;
    }
    int fd = open(path, O_RDWR | O_CREAT, 0644);
    if (fd < 0) {
        return SHEAF_WAL_EIO;
    }
    *out = (SheafFile)(intptr_t)fd;
    return SHEAF_WAL_OK;
}

int sheaf_platform_write(SheafFile file, uint64_t offset,
                         const void *data, size_t len) {
    int fd = (int)(intptr_t)file;
    const uint8_t *cursor = (const uint8_t *)data;
    size_t done = 0;
    while (done < len) {
        size_t chunk = len - done;
        if (chunk > SHEAF_POSIX_IO_CHUNK) {
            chunk = SHEAF_POSIX_IO_CHUNK;
        }
        ssize_t n = pwrite(fd, cursor + done, chunk, (off_t)(offset + done));
        if (n < 0) {
            if (errno == EINTR) {
                continue; /* A signal interrupted the write. Retry. */
            }
            return SHEAF_WAL_EIO;
        }
        if (n == 0) {
            return SHEAF_WAL_EIO; /* No progress means a broken device. */
        }
        done += (size_t)n;
    }
    return SHEAF_WAL_OK;
}

int sheaf_platform_read(SheafFile file, uint64_t offset,
                        void *data, size_t len) {
    int fd = (int)(intptr_t)file;
    uint8_t *cursor = (uint8_t *)data;
    size_t done = 0;
    while (done < len) {
        size_t chunk = len - done;
        if (chunk > SHEAF_POSIX_IO_CHUNK) {
            chunk = SHEAF_POSIX_IO_CHUNK;
        }
        ssize_t n = pread(fd, cursor + done, chunk, (off_t)(offset + done));
        if (n < 0) {
            if (errno == EINTR) {
                continue;
            }
            return SHEAF_WAL_EIO;
        }
        if (n == 0) {
            /* End of file inside a record means a torn tail. The
             * recovery layer treats this as corruption, not an
             * I/O failure, so report it as a short read. */
            return SHEAF_WAL_ECORRUPT;
        }
        done += (size_t)n;
    }
    return SHEAF_WAL_OK;
}

int sheaf_platform_fsync(SheafFile file) {
    int fd = (int)(intptr_t)file;
    while (fsync(fd) != 0) {
        if (errno == EINTR) {
            continue;
        }
        return SHEAF_WAL_EIO;
    }
    return SHEAF_WAL_OK;
}

int sheaf_platform_size(SheafFile file, uint64_t *out_size) {
    if (out_size == NULL) {
        return SHEAF_WAL_EINVAL;
    }
    int fd = (int)(intptr_t)file;
    struct stat st;
    if (fstat(fd, &st) != 0) {
        return SHEAF_WAL_EIO;
    }
    if (st.st_size < 0) {
        return SHEAF_WAL_EIO;
    }
    *out_size = (uint64_t)st.st_size;
    return SHEAF_WAL_OK;
}

int sheaf_platform_truncate(SheafFile file, uint64_t len) {
    int fd = (int)(intptr_t)file;
    while (ftruncate(fd, (off_t)len) != 0) {
        if (errno == EINTR) {
            continue;
        }
        return SHEAF_WAL_EIO;
    }
    return SHEAF_WAL_OK;
}

void sheaf_platform_close(SheafFile file) {
    int fd = (int)(intptr_t)file;
    /* close() can report EINTR, but the descriptor is gone on Linux
     * either way. A retry can close an unrelated reused descriptor.
     * The WAL flushes before close, so no data loss follows. */
    (void)close(fd);
}

#endif /* !defined(_WIN32) */