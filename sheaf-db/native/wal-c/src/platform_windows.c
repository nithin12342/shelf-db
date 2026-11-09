/*
 * platform_windows.c - Windows backend for the WAL file interface.
 *
 * Durability path: CreateFile with FILE_FLAG_WRITE_THROUGH pushes writes
 * toward the device, and FlushFileBuffers is the explicit barrier used at
 * commit time. Positioned writes use OVERLAPPED offsets on a synchronous
 * handle, which is the documented way to pwrite on Win32.
 *
 * This backend compiles only on _WIN32. The POSIX and io_uring backends
 * implement the same interface for Linux.
 */
#ifdef _WIN32

#include "wal_internal.h"

#define WIN32_LEAN_AND_MEAN
#include <windows.h>

struct SheafFile {
    HANDLE handle;
};

SheafFile* sheaf_platform_open(const char* path) {
    HANDLE h = CreateFileA(
        path,
        GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ,
        NULL,
        OPEN_ALWAYS,
        FILE_ATTRIBUTE_NORMAL | FILE_FLAG_WRITE_THROUGH,
        NULL);
    if (h == INVALID_HANDLE_VALUE) return NULL;
    SheafFile* f = (SheafFile*)malloc(sizeof(SheafFile));
    if (f == NULL) {
        CloseHandle(h);
        return NULL;
    }
    f->handle = h;
    return f;
}

/* Loop until every byte is written. A partial WriteFile is legal on Win32
 * and must not be treated as a complete positioned write. */
int sheaf_platform_write(SheafFile* f, uint64_t offset, const void* buf, size_t len) {
    const uint8_t* p = (const uint8_t*)buf;
    size_t done = 0;
    while (done < len) {
        OVERLAPPED ov;
        memset(&ov, 0, sizeof(ov));
        ov.Offset = (DWORD)(offset + done);
        ov.OffsetHigh = (DWORD)((offset + done) >> 32);
        DWORD written = 0;
        const DWORD chunk = (DWORD)((len - done) > 0x40000000u ? 0x40000000u : (len - done));
        if (!WriteFile(f->handle, p + done, chunk, &written, &ov)) return SHEAF_WAL_EIO;
        if (written == 0) return SHEAF_WAL_EIO;
        done += written;
    }
    return SHEAF_WAL_OK;
}

int sheaf_platform_read(SheafFile* f, uint64_t offset, void* buf, size_t len) {
    uint8_t* p = (uint8_t*)buf;
    size_t done = 0;
    while (done < len) {
        OVERLAPPED ov;
        memset(&ov, 0, sizeof(ov));
        ov.Offset = (DWORD)(offset + done);
        ov.OffsetHigh = (DWORD)((offset + done) >> 32);
        DWORD got = 0;
        const DWORD chunk = (DWORD)((len - done) > 0x40000000u ? 0x40000000u : (len - done));
        if (!ReadFile(f->handle, p + done, chunk, &got, &ov)) {
            /* EOF before the requested length is a short read. */
            if (GetLastError() == ERROR_HANDLE_EOF) return SHEAF_WAL_EIO;
            return SHEAF_WAL_EIO;
        }
        if (got == 0) return SHEAF_WAL_EIO;
        done += got;
    }
    return SHEAF_WAL_OK;
}

int sheaf_platform_fsync(SheafFile* f) {
    return FlushFileBuffers(f->handle) ? SHEAF_WAL_OK : SHEAF_WAL_EIO;
}

int64_t sheaf_platform_size(SheafFile* f) {
    LARGE_INTEGER size;
    if (!GetFileSizeEx(f->handle, &size)) return -1;
    return (int64_t)size.QuadPart;
}

int sheaf_platform_truncate(SheafFile* f, uint64_t len) {
    LARGE_INTEGER pos;
    pos.QuadPart = (LONGLONG)len;
    if (!SetFilePointerEx(f->handle, pos, NULL, FILE_BEGIN)) return SHEAF_WAL_EIO;
    if (!SetEndOfFile(f->handle)) return SHEAF_WAL_EIO;
    return SHEAF_WAL_OK;
}

void sheaf_platform_close(SheafFile* f) {
    if (f == NULL) return;
    CloseHandle(f->handle);
    free(f);
}

#endif /* _WIN32 */