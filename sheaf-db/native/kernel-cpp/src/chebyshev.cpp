// chebyshev.cpp - Chebyshev semi-iteration for L x = b (spec Pillar 1/6).
//
// Exact Saad three-term recurrence over the window [lambda_min, lambda_max]:
//   sigma = d/c, rho_0 = 1/sigma, rho_k = 1/(2 sigma - rho_{k-1})
//   beta_k = rho_{k-1} rho_k , alpha_0 = 1/d , alpha_k = (1 + beta_k)/d
//   x_{k+1} = x_k + alpha_k r_k + beta_k (x_k - x_{k-1})
// with d = (lmax + lmin)/2 and c = (lmax - lmin)/2. The inexact variant
// beta_k = (c alpha_{k-1}/2)^2 stalls near rel 0.04; the production Rust
// solver in sheaf-kernel recorded that failure, so this tier uses the
// exact rho form from the start.
//
// Contract: lambda_min must be a POSITIVE lower bound. A sheaf Laplacian
// has a zero eigenvalue (global sections). Callers either solve the
// Tikhonov-shifted system (L + mu I) with lambda_min = mu, or pass the
// smallest positive eigenvalue when b lies in range(L). With lmin = 0 the
// window ratio d/c equals 1 and the iteration is marginally stable.
//
// Scratch layout (all f32, caller owned):
//   [ r: n ][ x_prev: n ][ lx: n ][ edge cochain: m ]
//   n = num_vertices * d ,  m = num_edges * d

#include "internal.hpp"

namespace sheaf::kernel {
namespace {

// One combined update step. It reads the old iterate before overwriting it,
// so x and x_prev swap roles without a third full-size buffer.
void update_scalar(float* x, float* x_prev, const float* r,
                   float alpha, float beta, size_t n) {
    for (size_t i = 0; i < n; ++i) {
        const float old = x[i];
        x[i] = old + alpha * r[i] + beta * (old - x_prev[i]);
        x_prev[i] = old;
    }
}

SHEAF_TARGET_AVX2
void update_avx2(float* x, float* x_prev, const float* r,
                 float alpha, float beta, size_t n) {
    const __m256 va = _mm256_set1_ps(alpha);
    const __m256 vb = _mm256_set1_ps(beta);
    size_t i = 0;
    for (; i + 8 <= n; i += 8) {
        const __m256 old = _mm256_loadu_ps(x + i);
        const __m256 prev = _mm256_loadu_ps(x_prev + i);
        const __m256 rr = _mm256_loadu_ps(r + i);
        // next = old + alpha*r + beta*(old - prev)
        __m256 next = _mm256_fmadd_ps(va, rr, old);
        next = _mm256_fmadd_ps(vb, _mm256_sub_ps(old, prev), next);
        _mm256_storeu_ps(x + i, next);
        _mm256_storeu_ps(x_prev + i, old);
    }
    update_scalar(x + i, x_prev + i, r + i, alpha, beta, n - i);
}

void update_step(float* x, float* x_prev, const float* r,
                 float alpha, float beta, size_t n) {
#if SHEAF_X86 && SHEAF_TARGET_SIMD_OK
    if (has_avx2()) { update_avx2(x, x_prev, r, alpha, beta, n); return; }
#endif
    update_scalar(x, x_prev, r, alpha, beta, n);
}

}  // namespace
}  // namespace sheaf::kernel

using namespace sheaf::kernel;

extern "C" size_t sheaf_chebyshev_scratch_floats(const SheafComplexDesc* cx) {
    if (!desc_ok(cx)) return 0;
    const size_t n = static_cast<size_t>(cx->num_vertices) * cx->stalk_dim;
    const size_t m = static_cast<size_t>(cx->num_edges) * cx->stalk_dim;
    return 3 * n + m;
}

extern "C" int sheaf_chebyshev_solve(const SheafComplexDesc* cx, const float* b,
                                     float* x, uint32_t max_iters, float tol,
                                     float lambda_min, float lambda_max,
                                     float* scratch) {
    if (!desc_ok(cx) || b == nullptr || x == nullptr || scratch == nullptr)
        return SHEAF_KERNEL_EINVAL;
    if (!(lambda_min > 0.0f) || lambda_max <= lambda_min || !(tol > 0.0f))
        return SHEAF_KERNEL_EINVAL;

    const size_t n = static_cast<size_t>(cx->num_vertices) * cx->stalk_dim;
    const size_t m = static_cast<size_t>(cx->num_edges) * cx->stalk_dim;
    float* r = scratch;             // residual b - L x
    float* x_prev = scratch + n;    // previous iterate for the momentum term
    float* lx = scratch + 2 * n;    // operator image L x
    float* edge_scratch = scratch + 3 * n;  // pass-1 cochain, m floats
    (void)m;

    const float center = 0.5f * (lambda_max + lambda_min);
    const float half_width = 0.5f * (lambda_max - lambda_min);
    // Relative stopping rule: ||r|| <= tol * ||b||. The floor keeps the rule
    // well defined when b is the zero vector (then x must stay in ker L).
    const float b_norm = std::sqrt(dot(b, b, n));
    const float target = tol * (b_norm > 1e-30f ? b_norm : 1e-30f);
    const float target2 = target * target;

    const float sigma = center / half_width;
    float rho_old = 1.0f / sigma;
    const float alpha0 = 1.0f / center;
    bool first = true;

    for (uint32_t k = 0; k < max_iters; ++k) {
        // r = b - L x, with L applied through the factored two-pass path.
        const int rc = sheaf_laplacian_apply(cx, x, edge_scratch, lx);
        if (rc != SHEAF_KERNEL_OK) return rc;
        std::memcpy(r, b, n * sizeof(float));
        axpy(r, lx, -1.0f, n);
        if (dot(r, r, n) <= target2) return static_cast<int>(k);

        float beta = 0.0f;
        float alpha = alpha0;
        if (!first) {
            const float rho_new = 1.0f / (2.0f * sigma - rho_old);
            beta = rho_old * rho_new;
            alpha = (1.0f + beta) / center;
            rho_old = rho_new;
        }
        update_step(x, x_prev, r, alpha, beta, n);
        first = false;
    }
    return SHEAF_KERNEL_ENOCONV;
}