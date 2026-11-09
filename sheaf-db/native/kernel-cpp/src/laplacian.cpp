// laplacian.cpp - factored streaming sheaf Laplacian (spec Pillar 1).
//
// The dense operator L = delta* delta is never materialized. Each call runs
// two streaming passes over the edge list:
//   Pass 1 (coboundary):        v_e = R_dst s_dst - R_src s_src
//   Pass 2 (adjoint scatter):   out[src] -= R_src^T v_e, out[dst] += R_dst^T v_e
//
// Identity restrictions (null matrix pointers) reduce to oriented graph
// differences and take the fast subtract/axpy paths. Matrix restrictions use
// row-dot and broadcast-scatter kernels so no temporary d-vector is needed.

#include "internal.hpp"

namespace sheaf::kernel {
namespace {

// ---- vector subtract: out = a - b ----------------------------------------

void sub_scalar(float* out, const float* a, const float* b, size_t n) {
    for (size_t i = 0; i < n; ++i) out[i] = a[i] - b[i];
}

SHEAF_TARGET_AVX2
void sub_avx2(float* out, const float* a, const float* b, size_t n) {
    size_t i = 0;
    for (; i + 8 <= n; i += 8)
        _mm256_storeu_ps(out + i,
                         _mm256_sub_ps(_mm256_loadu_ps(a + i), _mm256_loadu_ps(b + i)));
    sub_scalar(out + i, a + i, b + i, n - i);
}

#if SHEAF_TARGET_SIMD_OK >= 2 || defined(__GNUC__) || defined(__clang__)
SHEAF_TARGET_AVX512
void sub_avx512(float* out, const float* a, const float* b, size_t n) {
    size_t i = 0;
    for (; i + 16 <= n; i += 16)
        _mm512_storeu_ps(out + i,
                         _mm512_sub_ps(_mm512_loadu_ps(a + i), _mm512_loadu_ps(b + i)));
    sub_avx2(out + i, a + i, b + i, n - i);
}
#endif

void sub_vec(float* out, const float* a, const float* b, size_t n) {
#if SHEAF_X86 && (SHEAF_TARGET_SIMD_OK >= 2 || defined(__GNUC__) || defined(__clang__))
    if (has_avx512()) { sub_avx512(out, a, b, n); return; }
#endif
#if SHEAF_X86 && SHEAF_TARGET_SIMD_OK
    if (has_avx2()) { sub_avx2(out, a, b, n); return; }
#endif
    sub_scalar(out, a, b, n);
}

// ---- row-dot accumulate: out[i] += alpha * (M row i) . in -----------------
// M is row-major d x d. This evaluates one restriction image without a
// temporary vector, which keeps the pass-1 working set inside L1.

void rowdot_scalar(float* out, const float* m, const float* in,
                   float alpha, uint32_t d) {
    for (uint32_t i = 0; i < d; ++i)
        out[i] += alpha * dot_scalar(m + static_cast<size_t>(i) * d, in, d);
}

SHEAF_TARGET_AVX2
void rowdot_avx2(float* out, const float* m, const float* in,
                 float alpha, uint32_t d) {
    for (uint32_t i = 0; i < d; ++i)
        out[i] += alpha * dot_avx2(m + static_cast<size_t>(i) * d, in, d);
}

void rowdot_accum(float* out, const float* m, const float* in,
                  float alpha, uint32_t d) {
#if SHEAF_X86 && SHEAF_TARGET_SIMD_OK
    if (has_avx2()) { rowdot_avx2(out, m, in, alpha, d); return; }
#endif
    rowdot_scalar(out, m, in, alpha, d);
}

// ---- transpose scatter: out[j] += alpha * sum_i M[i][j] * v[i] ------------
// The broadcast form walks M row by row, so the restriction block streams
// through cache once. This is the adjoint restriction R^T applied to v_e.

void tscatter_scalar(float* out, const float* m, const float* v,
                     float alpha, uint32_t d) {
    for (uint32_t i = 0; i < d; ++i) {
        const float w = alpha * v[i];
        const float* row = m + static_cast<size_t>(i) * d;
        for (uint32_t j = 0; j < d; ++j) out[j] += w * row[j];
    }
}

SHEAF_TARGET_AVX2
void tscatter_avx2(float* out, const float* m, const float* v,
                   float alpha, uint32_t d) {
    uint32_t i = 0;
    for (; i < d; ++i) {
        const __m256 w = _mm256_set1_ps(alpha * v[i]);
        const float* row = m + static_cast<size_t>(i) * d;
        uint32_t j = 0;
        for (; j + 8 <= d; j += 8)
            _mm256_storeu_ps(out + j,
                             _mm256_fmadd_ps(w, _mm256_loadu_ps(row + j),
                                             _mm256_loadu_ps(out + j)));
        for (; j < d; ++j) out[j] += alpha * v[i] * row[j];
    }
}

#if SHEAF_TARGET_SIMD_OK >= 2 || defined(__GNUC__) || defined(__clang__)
SHEAF_TARGET_AVX512
void tscatter_avx512(float* out, const float* m, const float* v,
                     float alpha, uint32_t d) {
    for (uint32_t i = 0; i < d; ++i) {
        const __m512 w = _mm512_set1_ps(alpha * v[i]);
        const float* row = m + static_cast<size_t>(i) * d;
        uint32_t j = 0;
        for (; j + 16 <= d; j += 16)
            _mm512_storeu_ps(out + j,
                             _mm512_fmadd_ps(w, _mm512_loadu_ps(row + j),
                                             _mm512_loadu_ps(out + j)));
        for (; j < d; ++j) out[j] += alpha * v[i] * row[j];
    }
}
#endif

void tscatter_accum(float* out, const float* m, const float* v,
                    float alpha, uint32_t d) {
#if SHEAF_X86 && (SHEAF_TARGET_SIMD_OK >= 2 || defined(__GNUC__) || defined(__clang__))
    if (has_avx512()) { tscatter_avx512(out, m, v, alpha, d); return; }
#endif
#if SHEAF_X86 && SHEAF_TARGET_SIMD_OK
    if (has_avx2()) { tscatter_avx2(out, m, v, alpha, d); return; }
#endif
    tscatter_scalar(out, m, v, alpha, d);
}

}  // namespace
}  // namespace sheaf::kernel

using namespace sheaf::kernel;

// Pass 1: v = delta s. One streaming sweep over the oriented edge list.
extern "C" int sheaf_coboundary_apply(const SheafComplexDesc* cx,
                                      const float* s, float* edge_out) {
    if (!desc_ok(cx) || s == nullptr || edge_out == nullptr)
        return SHEAF_KERNEL_EINVAL;
    const uint32_t d = cx->stalk_dim;
    const size_t dd = static_cast<size_t>(d) * d;
    const bool identity = cx->r_src == nullptr;
    for (uint32_t e = 0; e < cx->num_edges; ++e) {
        const float* src = s + static_cast<size_t>(cx->edge_src[e]) * d;
        const float* dst = s + static_cast<size_t>(cx->edge_dst[e]) * d;
        float* out = edge_out + static_cast<size_t>(e) * d;
        if (identity) {
            sub_vec(out, dst, src, d);
        } else {
            zero(out, d);
            rowdot_accum(out, cx->r_dst + e * dd, dst, +1.0f, d);
            rowdot_accum(out, cx->r_src + e * dd, src, -1.0f, d);
        }
    }
    return SHEAF_KERNEL_OK;
}

// Pass 2: out = delta* v. The output is zeroed, then edges scatter-add.
extern "C" int sheaf_coboundary_adjoint(const SheafComplexDesc* cx,
                                        const float* edge_vec, float* vertex_out) {
    if (!desc_ok(cx) || edge_vec == nullptr || vertex_out == nullptr)
        return SHEAF_KERNEL_EINVAL;
    const uint32_t d = cx->stalk_dim;
    const size_t dd = static_cast<size_t>(d) * d;
    const bool identity = cx->r_src == nullptr;
    zero(vertex_out, static_cast<size_t>(cx->num_vertices) * d);
    for (uint32_t e = 0; e < cx->num_edges; ++e) {
        const float* v = edge_vec + static_cast<size_t>(e) * d;
        float* src = vertex_out + static_cast<size_t>(cx->edge_src[e]) * d;
        float* dst = vertex_out + static_cast<size_t>(cx->edge_dst[e]) * d;
        if (identity) {
            axpy(dst, v, +1.0f, d);
            axpy(src, v, -1.0f, d);
        } else {
            tscatter_accum(dst, cx->r_dst + e * dd, v, +1.0f, d);
            tscatter_accum(src, cx->r_src + e * dd, v, -1.0f, d);
        }
    }
    return SHEAF_KERNEL_OK;
}

// Full factored application. The edge cochain lives in caller scratch only.
extern "C" int sheaf_laplacian_apply(const SheafComplexDesc* cx, const float* s,
                                     float* scratch_edges, float* out) {
    if (scratch_edges == nullptr || out == nullptr) return SHEAF_KERNEL_EINVAL;
    const int rc = sheaf_coboundary_apply(cx, s, scratch_edges);
    if (rc != SHEAF_KERNEL_OK) return rc;
    return sheaf_coboundary_adjoint(cx, scratch_edges, out);
}

// Energy = ||delta s||^2. This equals <s, L s> by the adjoint identity.
extern "C" float sheaf_energy(const SheafComplexDesc* cx, const float* s,
                              float* scratch_edges) {
    if (!desc_ok(cx) || s == nullptr || scratch_edges == nullptr) return -1.0f;
    if (sheaf_coboundary_apply(cx, s, scratch_edges) != SHEAF_KERNEL_OK)
        return -1.0f;
    const size_t n = static_cast<size_t>(cx->num_edges) * cx->stalk_dim;
    return dot(scratch_edges, scratch_edges, n);
}