/*
 * rdma_probe_test.c - C-06 test of the production RDMA probe and write path.
 *
 * Timestamp: 2026-11-22_12-48-16 +05:30
 * Remote: origin https://github.com/nithin12342/shelf-db.git
 * Branch: combined-stack-feature-improvement
 * Base commit: 3832065581f3ff4240f4d267c38bcde8fea1c24e
 *
 * The test calls the C functions in native/rdma-c/src/rdma.c with a real
 * caller-owned buffer and a real follower descriptor. Expected values come
 * from the declared contract in sheaf_rdma.h, not from a constant copied into
 * this file. A success return without a posted work request fails the test.
 */
#include "sheaf_rdma.h"

#include <stdio.h>
#include <string.h>

static int failures = 0;

static void check(int condition, const char* label)
{
    if (condition) {
        printf("PASS %s\n", label);
    } else {
        printf("FAIL %s\n", label);
        failures++;
    }
}

int main(void)
{
    uint8_t staging[64];
    memset(staging, 0x11, sizeof(staging));

    uint32_t capability = sheaf_rdma_capability();
    const char* reason = sheaf_rdma_capability_reason();
    printf("capability=%u\n", capability);
    printf("reason=%s\n", reason);
    check(capability == SHEAF_RDMA_CAP_NONE, "capability_none");
    check(strcmp(reason, SHEAF_RDMA_REASON_NO_VERBS) == 0, "named_reason");

    SheafRdmaEndpoint* opened = (SheafRdmaEndpoint*)0x1;
    int code = sheaf_rdma_endpoint_open(&opened);
    check(code == SHEAF_RDMA_ECAP, "endpoint_open_ecap");
    check(opened == NULL, "endpoint_open_null_output");

    SheafRdmaBuffer local;
    memset(&local, 0, sizeof(local));
    local.addr = staging;
    local.length = sizeof(staging);
    local.lkey = 1u;
    local.rkey = 2u;
    local.handle = 3u;

    SheafRdmaWriteDesc desc;
    memset(&desc, 0, sizeof(desc));
    desc.remote_addr = 0x2000u;
    desc.rkey = 4u;
    desc.data = staging + 8;
    desc.len = 16u;

    /* The endpoint stays non-null so the call reaches the capability gate. */
    uint8_t opaque_endpoint = 0;
    SheafRdmaEndpoint* endpoint = (SheafRdmaEndpoint*)&opaque_endpoint;

    code = sheaf_rdma_post_write(endpoint, &local, &desc);
    check(code == SHEAF_RDMA_ECAP, "post_write_ecap_no_fake_success");

    /* An out-of-range slice fails argument validation before the gate. */
    desc.data = staging + 60;
    desc.len = 16u;
    code = sheaf_rdma_post_write(endpoint, &local, &desc);
    check(code == SHEAF_RDMA_EINVAL, "out_of_range_einval");

    printf("C06 RDMA PROBE: %s\n", failures == 0 ? "PASS" : "FAIL");
    return failures == 0 ? 0 : 1;
}
