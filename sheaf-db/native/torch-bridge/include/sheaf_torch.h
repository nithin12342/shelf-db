/*
 * sheaf_torch.h - C ABI boundary for the PyTorch bridge (spec Pillar 11).
 *
 * The bridge exposes BCSF stalk memory directly as a PyTorch tensor without
 * copying. The Rust/Zig storage tier owns the arena and implements these
 * accessors. The C++ extension (sheaf_torch.cpp) calls them to build a
 * zero-copy torch::from_blob view.
 *
 * Lifetime rule: the SheafHandle and its arena must outlive every tensor view
 * and every autograd graph that references the stalk memory. The epoch lease
 * in sheaf-runtime pins the arena for that lifetime (spec Pillar 4).
 */
#ifndef SHEAF_TORCH_H
#define SHEAF_TORCH_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque handle owned by the storage tier. */
typedef struct SheafHandle SheafHandle;

/* Aligned pointer to the stalk of `cell_id`. The memory stays owned by the
 * arena. Returns NULL when the cell is absent. */
float* sheaf_handle_stalk_ptr(SheafHandle* handle, uint64_t cell_id);

/* Number of f32 lanes in the stalk of `cell_id`. Returns 0 when absent. */
int64_t sheaf_handle_stalk_dim(SheafHandle* handle, uint64_t cell_id);

/* Apply the coboundary adjoint delta* (implemented in sheaf-kernel) to map an
 * edge gradient back to a vertex gradient. Used by the backward pass.
 *   grad_edge:   input gradient on the 1-cochain (edges lanes)
 *   grad_vertex: output gradient on the 0-cochain (vertex lanes)
 */
void sheaf_handle_coboundary_adjoint(SheafHandle* handle,
                                     const float* grad_edge,
                                     float* grad_vertex,
                                     uint64_t edge_lanes,
                                     uint64_t vertex_lanes);

#ifdef __cplusplus
}
#endif

#endif /* SHEAF_TORCH_H */