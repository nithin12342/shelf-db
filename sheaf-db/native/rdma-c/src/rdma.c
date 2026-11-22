/*
 * rdma.c - C11 RoCEv2 replication transport (spec line 316).
 *
 * Timestamp: 2026-11-22_12-48-16 +05:30
 * Remote: origin https://github.com/nithin12342/shelf-db.git
 * Branch: combined-stack-feature-improvement
 * Base commit: 3832065581f3ff4240f4d267c38bcde8fea1c24e
 *
 * The build defines SHEAF_HAVE_LIBIBVERBS only when the verbs headers and the
 * library are present. Without that definition this file still compiles as
 * C11, the probe returns SHEAF_RDMA_CAP_NONE, and every data-path call returns
 * SHEAF_RDMA_ECAP. No function writes bytes or returns OK in that state.
 */
#include "sheaf_rdma.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#if defined(SHEAF_HAVE_LIBIBVERBS)
#include <infiniband/verbs.h>
#endif

/* The provider objects owned by one endpoint. */
struct SheafRdmaEndpoint {
#if defined(SHEAF_HAVE_LIBIBVERBS)
    struct ibv_context* context;
    struct ibv_pd* pd;
    struct ibv_cq* cq;
    struct ibv_qp* qp;
    uint64_t writes_posted;
#else
    uint8_t opaque;
#endif
};

#if defined(SHEAF_HAVE_LIBIBVERBS)
/*
 * Find the first port whose link layer is Ethernet. RoCEv2 runs over Ethernet,
 * so an InfiniBand link layer does not qualify. The caller closes the returned
 * context. A return of zero means no qualifying port exists.
 */
static int rdma_find_ethernet_port(struct ibv_context** out_context)
{
    int count = 0;
    int found = 0;
    struct ibv_device** devices = ibv_get_device_list(&count);

    if (devices == NULL)
        return 0;

    for (int i = 0; i < count; ++i) {
        struct ibv_context* context = ibv_open_device(devices[i]);
        if (context == NULL)
            continue;

        struct ibv_port_attr attributes;
        memset(&attributes, 0, sizeof(attributes));
        if (ibv_query_port(context, 1u, &attributes) == 0 &&
            attributes.link_layer == IBV_LINK_LAYER_ETHERNET) {
            *out_context = context;
            found = 1;
            break;
        }
        ibv_close_device(context);
    }

    ibv_free_device_list(devices);
    return found;
}
#endif

uint32_t sheaf_rdma_capability(void)
{
#if defined(SHEAF_HAVE_LIBIBVERBS)
    struct ibv_context* context = NULL;
    if (rdma_find_ethernet_port(&context)) {
        ibv_close_device(context);
        return SHEAF_RDMA_CAP_ROCEV2;
    }
#endif
    return SHEAF_RDMA_CAP_NONE;
}

const char* sheaf_rdma_capability_reason(void)
{
#if defined(SHEAF_HAVE_LIBIBVERBS)
    if (sheaf_rdma_capability() == SHEAF_RDMA_CAP_ROCEV2)
        return "";
    return SHEAF_RDMA_REASON_NO_DEVICE;
#else
    return SHEAF_RDMA_REASON_NO_VERBS;
#endif
}

int sheaf_rdma_endpoint_open(SheafRdmaEndpoint** out)
{
    if (out == NULL)
        return SHEAF_RDMA_EINVAL;
    *out = NULL;

    if (sheaf_rdma_capability() != SHEAF_RDMA_CAP_ROCEV2)
        return SHEAF_RDMA_ECAP;

#if defined(SHEAF_HAVE_LIBIBVERBS)
    struct ibv_context* context = NULL;
    if (!rdma_find_ethernet_port(&context))
        return SHEAF_RDMA_ECAP;

    struct ibv_pd* pd = ibv_alloc_pd(context);
    if (pd == NULL) {
        ibv_close_device(context);
        return SHEAF_RDMA_ENOMEM;
    }

    struct ibv_cq* cq = ibv_create_cq(context, 64, NULL, NULL, 0);
    if (cq == NULL) {
        ibv_dealloc_pd(pd);
        ibv_close_device(context);
        return SHEAF_RDMA_ENOMEM;
    }

    struct ibv_qp_init_attr init;
    memset(&init, 0, sizeof(init));
    init.qp_type = IBV_QPT_RC;
    init.send_cq = cq;
    init.recv_cq = cq;
    init.cap.max_send_wr = 64;
    init.cap.max_send_sge = 1;
    init.cap.max_recv_wr = 1;
    init.cap.max_recv_sge = 1;

    struct ibv_qp* qp = ibv_create_qp(pd, &init);
    if (qp == NULL) {
        ibv_destroy_cq(cq);
        ibv_dealloc_pd(pd);
        ibv_close_device(context);
        return SHEAF_RDMA_ENOMEM;
    }

    SheafRdmaEndpoint* endpoint = (SheafRdmaEndpoint*)calloc(1, sizeof(*endpoint));
    if (endpoint == NULL) {
        ibv_destroy_qp(qp);
        ibv_destroy_cq(cq);
        ibv_dealloc_pd(pd);
        ibv_close_device(context);
        return SHEAF_RDMA_ENOMEM;
    }

    /* The cluster handshake moves this queue pair to RTS and exchanges the
     * remote key before any write is posted. Post-send then fails with EIO
     * if the handshake has not run. No call invents a ready queue pair. */
    endpoint->context = context;
    endpoint->pd = pd;
    endpoint->cq = cq;
    endpoint->qp = qp;
    endpoint->writes_posted = 0;
    *out = endpoint;
    return SHEAF_RDMA_OK;
#else
    return SHEAF_RDMA_ECAP;
#endif
}

int sheaf_rdma_endpoint_close(SheafRdmaEndpoint* endpoint)
{
    if (endpoint == NULL)
        return SHEAF_RDMA_OK;

#if defined(SHEAF_HAVE_LIBIBVERBS)
    if (endpoint->qp != NULL)
        ibv_destroy_qp(endpoint->qp);
    if (endpoint->cq != NULL)
        ibv_destroy_cq(endpoint->cq);
    if (endpoint->pd != NULL)
        ibv_dealloc_pd(endpoint->pd);
    if (endpoint->context != NULL)
        ibv_close_device(endpoint->context);
#endif
    free(endpoint);
    return SHEAF_RDMA_OK;
}

int sheaf_rdma_register(SheafRdmaEndpoint* endpoint, SheafRdmaBuffer* out,
                        void* addr, size_t length)
{
    if (endpoint == NULL || out == NULL || addr == NULL || length == 0)
        return SHEAF_RDMA_EINVAL;
    if (sheaf_rdma_capability() != SHEAF_RDMA_CAP_ROCEV2)
        return SHEAF_RDMA_ECAP;

#if defined(SHEAF_HAVE_LIBIBVERBS)
    struct ibv_mr* region = ibv_reg_mr(
        endpoint->pd, addr, length,
        IBV_ACCESS_LOCAL_WRITE | IBV_ACCESS_REMOTE_WRITE | IBV_ACCESS_REMOTE_READ);
    if (region == NULL)
        return SHEAF_RDMA_EIO;

    out->addr = (uint8_t*)addr;
    out->length = length;
    out->lkey = region->lkey;
    out->rkey = region->rkey;
    out->handle = (uint64_t)(uintptr_t)region;
    return SHEAF_RDMA_OK;
#else
    (void)endpoint;
    (void)out;
    (void)addr;
    (void)length;
    return SHEAF_RDMA_ECAP;
#endif
}

int sheaf_rdma_deregister(SheafRdmaEndpoint* endpoint, SheafRdmaBuffer* buffer)
{
    if (endpoint == NULL || buffer == NULL || buffer->handle == 0)
        return SHEAF_RDMA_EINVAL;
    if (sheaf_rdma_capability() != SHEAF_RDMA_CAP_ROCEV2)
        return SHEAF_RDMA_ECAP;

#if defined(SHEAF_HAVE_LIBIBVERBS)
    struct ibv_mr* region = (struct ibv_mr*)(uintptr_t)buffer->handle;
    if (ibv_dereg_mr(region) != 0)
        return SHEAF_RDMA_EIO;
    memset(buffer, 0, sizeof(*buffer));
    return SHEAF_RDMA_OK;
#else
    (void)endpoint;
    (void)buffer;
    return SHEAF_RDMA_ECAP;
#endif
}

int sheaf_rdma_post_write(SheafRdmaEndpoint* endpoint,
                          const SheafRdmaBuffer* local,
                          const SheafRdmaWriteDesc* desc)
{
    if (endpoint == NULL || local == NULL || desc == NULL ||
        local->addr == NULL || desc->data == NULL || desc->len == 0)
        return SHEAF_RDMA_EINVAL;
    if (desc->len > UINT32_MAX)
        return SHEAF_RDMA_EINVAL;

    /* Check that the delta slice lies inside the registered buffer. The
     * subtraction is safe after the lower-bound test. */
    uintptr_t base = (uintptr_t)local->addr;
    uintptr_t start = (uintptr_t)desc->data;
    if (start < base || start - base > local->length ||
        desc->len > local->length - (start - base))
        return SHEAF_RDMA_EINVAL;

    /* Return the named capability error before any verbs pointer is used. */
    if (sheaf_rdma_capability() != SHEAF_RDMA_CAP_ROCEV2)
        return SHEAF_RDMA_ECAP;

#if defined(SHEAF_HAVE_LIBIBVERBS)
    struct ibv_sge entry;
    memset(&entry, 0, sizeof(entry));
    entry.addr = (uint64_t)start;
    entry.length = (uint32_t)desc->len;
    entry.lkey = local->lkey;

    struct ibv_send_wr request;
    memset(&request, 0, sizeof(request));
    struct ibv_send_wr* bad = NULL;
    request.wr_id = desc->remote_addr;
    request.opcode = IBV_WR_RDMA_WRITE;
    request.send_flags = IBV_SEND_SIGNALED;
    request.sg_list = &entry;
    request.num_sge = 1;
    request.wr.rdma.remote_addr = desc->remote_addr;
    request.wr.rdma.rkey = desc->rkey;

    if (ibv_post_send(endpoint->qp, &request, &bad) != 0)
        return SHEAF_RDMA_EIO;
    endpoint->writes_posted++;
    return SHEAF_RDMA_OK;
#else
    (void)endpoint;
    (void)local;
    (void)desc;
    return SHEAF_RDMA_ECAP;
#endif
}
