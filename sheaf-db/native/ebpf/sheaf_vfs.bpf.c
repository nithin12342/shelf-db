// A BPF program skeleton for VFS interception (NF-11).
//
// This source is not compiled on this Windows host: clang with the BPF target
// and libbpf are absent. It is kept as the native-side contract that a Linux
// loader would attach. The Rust policy engine and the explicit Unsupported
// loader are the parts that this build can verify.

// SPDX-License-Identifier: GPL-2.0

#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

// One event from the VFS tracepoint.
struct vfs_event {
    __u32 op;       // 0 read, 1 write, 2 execute
    __u32 path_len;
    char path[256];
};

// The decision returned to the loader ring buffer.
struct vfs_decision {
    __u32 event_id;
    __u32 decision; // 0 allow, 1 deny protected, 2 deny read-only
};

// The policy flags are written once by the userspace loader.
struct {
    __uint(type, BPF_MAP_TYPE_ARRAY);
    __uint(max_entries, 1);
    __type(key, __u32);
    __type(value, __u32); // bit 0: read-only, bit 1: protected-prefix mode
} sheaf_vfs_policy SEC(".maps");

// The ring buffer that carries decisions to userspace.
struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 256 * 1024);
} sheaf_vfs_events SEC(".maps");

SEC("tracepoint/syscalls/sys_enter_openat")
int sheaf_vfs_openat(struct vfs_event *ctx)
{
    struct vfs_decision *out;
    __u32 key = 0;
    __u32 *flags = bpf_map_lookup_elem(&sheaf_vfs_policy, &key);
    __u32 decision = 0;

    if (flags && ((*flags & 1U) && ctx->op == 1U))
        decision = 2;
    if (flags && (*flags & 2U) && ctx->path_len > 0)
        decision = 1;

    out = bpf_ringbuf_reserve(&sheaf_vfs_events, sizeof(*out), 0);
    if (!out)
        return 0;
    out->event_id = 0;
    out->decision = decision;
    bpf_ringbuf_submit(out, 0);
    return 0;
}

char _license[] SEC("license") = "GPL";
