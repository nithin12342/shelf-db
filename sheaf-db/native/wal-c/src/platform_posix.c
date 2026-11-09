/*
 * platform_posix.c - POSIX backend for the WAL file interface.
 *
 * Maps the wal_internal.h file interface onto open(2), pwrite(2),
 * pread(2), fsync(2), fstat(2), ftruncate(2), and close(2).
 *
 * The io_uring backend (platform_uring.c) owns the write symbol on
 * Linux when SHEAF_WAL_URING selects it. This file then compiles its
 * write out, so the link sees exactly one definition. All other
 * operations stay here because they are off the hot append path.
 */
#if !defined(_WIN32)

#define _POSIX_C_SOURCE 200809L

#include "wal_internal.h"

#include <errno.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <unistd.h>

struct SheafFile {
    int fd;
};

/* Write chunk cap. A single pwrite of a very large buffer can return
 * a short count on some kernels. Bounded chunks keep the loop simple
 * and avoid implementation-specific limits. */
#define SHEAF_POSIX_IO_CHUNK 0x40000000u

SheafFile* sheaf_platform_open(const char* path) {
    if (path == NULL) return NULL;
    int fd = open(path, O_RDWR | O_CREAT, 0644);
    if (fd < 0) return NULL;
    SheafFile* f = (SheafFile*)malloc(sizeof(SheafFile));
    if (f == NULL) {
        (void)close(fd);
        return NULL;
    }
    f->fd = fd;
    return f;
}

/* The io_uring backend owns the write symbol when it is selected at
 * build time. Guard this definition so the link sees exactly one. */
#if !(defined(__linux__) && defined(SHEAF_WAL_URING))
int sheaf_platform_write(SheafFile* f, uint64_t offset, const void* buf, size_t len) {
    const uint8_t* p = (const uint8_t*)buf;
    size_t done = 0;
    while (done < len) {
        size_t chunk = len - done;
        if (chunk > SHEAF_POSIX_IO_CHUNK) chunk = SHEAF_POSIX_IO_CHUNK;
        ssize_t n = pwrite(f->fd, p + done, chunk, (off_t)(offset + done));
        if (n < 0) {
            if (errno == EINTR) continue; /* A signal interrupted us. */
            return SHEAF_WAL_EIO;
        }
        if (n == 0) return SHEAF_WAL_EIO; /* No progress: broken device. */
        done += (size_t)n;
    }
    return SHEAF_WAL_OK;
}
#endif /* !(defined(__linux__) && defined(SHEAF_WAL_URING)) */

/* Exact-length read. A short read returns SHEAF_WAL_EIO per the
 * interface contract. Recovery checks the file size first, so EOF
 * inside a record surfaces as a size mismatch before this call. */
int sheaf_platform_read(SheafFile* f, uint64_t offset, void* buf, size_t len) {
    uint8_t* p = (uint8_t*)buf;
    size_t done = 0;
    while (done < len) {
        size_t chunk = len - done;
        if (chunk > SHEAF_POSIX_IO_CHUNK) chunk = SHEAF_POSIX_IO_CHUNK;
        ssize_t n = pread(f->fd, p + done, chunk, (off_t)(offset + done));
        if (n < 0) {
            if (errno == EINTR) continue;
            return SHEAF_WAL_EIO;
        }
        if (n == 0) return SHEAF_WAL_EIO;
        done += (size_t)n;
    }
    return SHEAF_WAL_OK;
}

int sheaf_platform_fsync(SheafFile* f) {
    while (fsync(f->fd) != 0) {
        if (errno == EINTR) continue;
        return SHEAF_WAL_EIO;
    }
    return SHEAF_WAL_OK;
}

int64_t sheaf_platform_size(SheafFile* f) {
    struct stat st;
    if (fstat(f->fd, &st) != 0) return -1;
    if (st.st_size < 0) return -1;
    return (int64_t)st.st_size;
}

int sheaf_platform_truncate(SheafFile* f, uint64_t len) {
    while (ftruncate(f->fd, (off_t)len) != 0) {
        if (errno == EINTR) continue;
        return SHEAF_WAL_EIO;
    }
    return SHEAF_WAL_OK;
}

void sheaf_platform_close(SheafFile* f) {
    if (f == NULL) return;
    /* close() can report EINTR, but the descriptor is gone on Linux
     * either way. A retry can close an unrelated reused descriptor.
     * The WAL flushes before close, so no data loss follows. */
    (void)close(f->fd);
    free(f);
}

#endif /* !defined(_WIN32) */