/*
 * sheaf_rdma.h - C11 ABI for zero-copy RoCEv2 replication (spec line 316).
 *
 * Timestamp: 2026-11-22_12-48-16 +05:30
 * Remote: origin https://github.com/nithin12342/shelf-db.git
 * Branch: combined-stack-feature-improvement
 * Base commit: 3832065581f3ff4240f4d267c38bcde8fea1c24e
 *
 * Purpose. A shard leader writes sparse 0-cochain XOR deltas directly into a
 * follower's pre-registered memory buffer. The data path posts one RDMA write
 * work request. The CPU does not serialize a delta into a socket frame.
 *
 * Capability rule. The probe never reports success from a guess. When this
 * build has no libibverbs, the probe returns SHEAF_RDMA_CAP_NONE and the reason
 * string names the missing library. When libibverbs is present but no Ethernet
 * RDMA port is present, the probe returns SHEAF_RDMA_CAP_NONE with a different
 * named reason. Every data-path call returns SHEAF_RDMA_ECAP when capability is
 * none. No call returns SHEAF_RDMA_OK without a posted work request.
 *
 * Buffer lifecycle. The caller registers one local staging buffer, then reuses
 * it for every delta. A write descriptor names the remote virtual address and
 * the remote key of the follower's registered buffer.
 */
#ifndef SHEAF_RDMA_H
#define SHEAF_RDMA_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Values returned by sheaf_rdma_capability. */
#define SHEAF_RDMA_CAP_NONE    0u
#define SHEAF_RDMA_CAP_ROCEV2  1u

/* Return codes for every data-path function. Zero means success. */
#define SHEAF_RDMA_OK       0
#define SHEAF_RDMA_EINVAL  (-1) /* bad argument or out-of-range buffer slice */
#define SHEAF_RDMA_ECAP    (-2) /* the host has no proved RDMA capability    */
#define SHEAF_RDMA_EIO     (-3) /* the verbs provider rejected the request  */
#define SHEAF_RDMA_ENOMEM  (-4) /* a verbs object could not be allocated    */

/* Named reasons. The probe stores the active one for capability_reason. */
#define SHEAF_RDMA_REASON_NO_VERBS \
    "libibverbs is not available on this host (SHEAF_HAVE_LIBIBVERBS is not defined)"
#define SHEAF_RDMA_REASON_NO_DEVICE \
    "no Ethernet RDMA RoCEv2 port is present on this host"

/*
 * One local buffer registered with the verbs provider. The provider writes the
 * lkey and the rkey into this record. The handle field is opaque to Rust.
 */
typedef struct SheafRdmaBuffer {
    uint8_t* addr;   /* base address of the registered region       */
    size_t length;   /* registered byte count                       */
    uint32_t lkey;   /* local key for the scatter-gather entry      */
    uint32_t rkey;   /* remote key shared with the follower         */
    uint64_t handle; /* provider handle, opaque to callers          */
} SheafRdmaBuffer;

/*
 * One one-sided write request. data points into a registered SheafRdmaBuffer.
 * remote_addr and rkey name the follower's pre-registered destination region.
 */
typedef struct SheafRdmaWriteDesc {
    uint64_t remote_addr;
    uint32_t rkey;
    const uint8_t* data;
    size_t len;
} SheafRdmaWriteDesc;

/* An endpoint owns the verbs context, protection domain, and queue pair. */
typedef struct SheafRdmaEndpoint SheafRdmaEndpoint;

/*
 * Probe this build and this host. The probe does not allocate a staging buffer
 * and does not post a work request. It opens each RDMA device only to query the
 * link layer of port 1, then closes the device before it returns.
 */
uint32_t sheaf_rdma_capability(void);

/*
 * The active named reason, or an empty string when capability is available.
 * The pointer stays valid for the process life.
 */
const char* sheaf_rdma_capability_reason(void);

/* Open an endpoint on the first RoCEv2 port. Returns ECAP when capability is
 * none. On success *out owns an endpoint that the caller must close. */
int sheaf_rdma_endpoint_open(SheafRdmaEndpoint** out);

/* Close an endpoint. A null pointer is a successful no-op. */
int sheaf_rdma_endpoint_close(SheafRdmaEndpoint* endpoint);

/* Register a caller-owned buffer. ECAP means no capability; EINVAL means a bad
 * address or length. The buffer stays valid until deregistration. */
int sheaf_rdma_register(SheafRdmaEndpoint* endpoint, SheafRdmaBuffer* out,
                        void* addr, size_t length);

/* Release a registered buffer. A null pointer is a successful no-op. */
int sheaf_rdma_deregister(SheafRdmaEndpoint* endpoint, SheafRdmaBuffer* buffer);

/*
 * Post one RDMA write of desc->data into the follower's registered buffer.
 * The slice must lie inside local->addr for local->length bytes. The function
 * returns ECAP before it touches any pointer when capability is none. It
 * returns OK only after ibv_post_send accepts the work request.
 */
int sheaf_rdma_post_write(SheafRdmaEndpoint* endpoint,
                          const SheafRdmaBuffer* local,
                          const SheafRdmaWriteDesc* desc);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SHEAF_RDMA_H */
