/*
 * sheaf_kernel.h - C ABI contract between the Rust orchestration tier (Tier 3)
 * and the C++23 vector compute tier (Tier 1).
 *
 * Design rules:
 *   1. Every symbol uses C linkage so Rust, Zig, and C callers share one ABI.
 *   2. The caller owns every buffer. The kernel never allocates memory.
 *   3. All stalk vectors are contiguous f32 streams (spec: precision down-scaling).
 *   4. The complex descriptor borrows caller arrays for one call only.
 *
 * The factored Laplacian never materializes the dense matrix L = delta* delta.
 * Pass 1 evaluates v = delta s on edges. Pass 2 scatters r = delta* v to cells.
 */
#ifndef SHEAF_KERNEL_H
#define SHEAF_KERNEL_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Runtime capability bits reported by sheaf_kernel_cpu_features(). */
#define SHEAF_CPU_AVX512F 0x1u
#define SHEAF_CPU_AVX2    0x2u
#define SHEAF_CPU_NEON    0x4u

/* Error codes. Negative return values signal failure. */
#define SHEAF_KERNEL_OK          0
#define SHEAF_KERNEL_EINVAL    (-1)  /* null pointer or inconsistent dimensions */
#define SHEAF_KERNEL_ESCRATCH  (-2)  /* scratch arena too small                 */
#define SHEAF_KERNEL_ENOCONV   (-3)  /* solver hit max_iters without tolerance  */

/*
 * Borrowed descriptor of a cellular sheaf with a uniform stalk dimension.
 *
 * Edges are oriented 1-cells e = (src -> dst). The coboundary on one edge is
 *   (delta s)_e = R_dst[e] * s_dst - R_src[e] * s_src
 * where R_src and R_dst are row-major d x d restriction blocks. When both
 * pointers are null the restriction maps are identities and the operator
 * reduces to the oriented graph incidence difference s_dst - s_src.
 */
typedef struct SheafComplexDesc {
    uint32_t num_vertices;      /* number of 0-cells                          */
    uint32_t num_edges;         /* number of oriented 1-cells                 */
    uint32_t stalk_dim;         /* uniform stalk dimension d, must be >= 1    */
    const uint32_t* edge_src;   /* [num_edges] source cell index per edge     */
    const uint32_t* edge_dst;   /* [num_edges] destination cell index per edge*/
    const float* r_src;         /* [num_edges * d * d] or null for identity   */
    const float* r_dst;         /* [num_edges * d * d] or null for identity   */
} SheafComplexDesc;

/* Report the instruction-set extensions available on this CPU. */
uint32_t sheaf_kernel_cpu_features(void);

/* Return the ABI version of this kernel library (currently 1). */
uint32_t sheaf_kernel_abi_version(void);

/*
 * Pass 1 of the factored Laplacian: edge cochain v = delta s.
 * s has num_vertices * d floats. edge_out has num_edges * d floats.
 */
int sheaf_coboundary_apply(const SheafComplexDesc* cx,
                           const float* s, float* edge_out);

/*
 * Pass 2 of the factored Laplacian: vertex cochain out = delta* v.
 * The output is zeroed first, then each edge scatters its contribution:
 *   out[src] -= R_src[e]^T v_e ,  out[dst] += R_dst[e]^T v_e
 */
int sheaf_coboundary_adjoint(const SheafComplexDesc* cx,
                             const float* edge_vec, float* vertex_out);

/*
 * Full factored application out = L s = delta* (delta s).
 * scratch_edges must hold num_edges * d floats. No dense matrix is built.
 */
int sheaf_laplacian_apply(const SheafComplexDesc* cx, const float* s,
                          float* scratch_edges, float* out);

/*
 * Dirichlet energy <s, L s> = ||delta s||^2, computed without materializing L.
 * scratch_edges must hold num_edges * d floats (same contract as the apply).
 */
float sheaf_energy(const SheafComplexDesc* cx, const float* s,
                   float* scratch_edges);

/*
 * Sparse XOR 0-cochain delta streams (spec Pillar 1).
 * fold:   out = base XOR deltas[0] XOR ... XOR deltas[num_deltas - 1]
 * encode: delta_out = base XOR target
 * All buffers hold len bytes and may not overlap unless documented.
 */
int sheaf_xor_delta_fold(const uint8_t* base, const uint8_t* const* deltas,
                         uint32_t num_deltas, size_t len, uint8_t* out);
int sheaf_xor_delta_encode(const uint8_t* base, const uint8_t* target,
                           size_t len, uint8_t* delta_out);

/* Morton (Z-order) interleave of two 32-bit coordinates into one 64-bit key. */
uint64_t sheaf_morton_encode(uint32_t x, uint32_t y);
void sheaf_morton_decode(uint64_t key, uint32_t* x, uint32_t* y);

/*
 * Cache-blocked matrix-vector product. The matrix is row-major rows x cols.
 * The loop walks 64 x 64 tiles in Morton order so each tile stays in L1.
 */
int sheaf_matvec_tiled(const float* matrix, uint32_t rows, uint32_t cols,
                       const float* x, float* out);

/*
 * Branch-free validity compaction. mask_bits holds one bit per input slot
 * (little-endian bit order, word i covers slots 64*i .. 64*i+63).
 * Valid values are copied contiguously into out. Returns the kept count.
 */
uint32_t sheaf_compact_valid(const float* values, const uint64_t* mask_bits,
                             uint32_t count, float* out);

/* Number of floats the Chebyshev solver needs in its scratch arena. */
size_t sheaf_chebyshev_scratch_floats(const SheafComplexDesc* cx);

/*
 * Chebyshev semi-iteration for L x = b with spectral bounds
 * [lambda_min, lambda_max]. x must hold the initial guess on entry.
 * scratch holds sheaf_chebyshev_scratch_floats(cx) floats.
 *
 * lambda_min must be positive. A sheaf Laplacian has a zero eigenvalue,
 * so callers either solve the Tikhonov-shifted system (L + mu I) with
 * lambda_min = mu, or pass the smallest positive eigenvalue when b lies
 * in range(L). The recurrence is the exact Saad rho form.
 *
 * Returns the iteration count on convergence, or a negative error code.
 */
int sheaf_chebyshev_solve(const SheafComplexDesc* cx, const float* b,
                          float* x, uint32_t max_iters, float tol,
                          float lambda_min, float lambda_max, float* scratch);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SHEAF_KERNEL_H */