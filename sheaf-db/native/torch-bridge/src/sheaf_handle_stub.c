/*
 * sheaf_handle_stub.c - demo arena for the PyTorch bridge.
 *
 * This is a stand-in for the real storage tier. It provides the sheaf_handle_*
 * symbols so the extension links and the demo runs without the Zig arena. In
 * production the storage tier implements these symbols over BCSF memory and
 * this file is removed from the build (see setup.py).
 *
 * The demo arena holds four 4-lane stalks. The adjoint reference assumes a line
 * graph where edge e joins vertex e and vertex e+1, matching the coboundary
 * (delta x)_e = x[e+1] - x[e].
 */
#include "sheaf_torch.h"

#include <stddef.h>
#include <string.h>

#define DEMO_CELLS 4
#define DEMO_DIM 4

static float demo_arena[DEMO_CELLS * DEMO_DIM];

struct SheafHandle {
    int unused;
};

float* sheaf_handle_stalk_ptr(SheafHandle* handle, uint64_t cell_id) {
    (void)handle;
    if (cell_id >= DEMO_CELLS) {
        return NULL;
    }
    return &demo_arena[cell_id * DEMO_DIM];
}

int64_t sheaf_handle_stalk_dim(SheafHandle* handle, uint64_t cell_id) {
    (void)handle;
    return cell_id < DEMO_CELLS ? DEMO_DIM : 0;
}

void sheaf_handle_coboundary_adjoint(SheafHandle* handle,
                                     const float* grad_edge,
                                     float* grad_vertex,
                                     uint64_t edge_lanes,
                                     uint64_t vertex_lanes) {
    (void)handle;
    /* delta* for the line graph: scatter +g to v=e+1 and -g to u=e. */
    memset(grad_vertex, 0, vertex_lanes * sizeof(float));
    for (uint64_t e = 0; e < edge_lanes; ++e) {
        if (e + 1 < vertex_lanes) {
            grad_vertex[e + 1] += grad_edge[e];
        }
        if (e < vertex_lanes) {
            grad_vertex[e] -= grad_edge[e];
        }
    }
}