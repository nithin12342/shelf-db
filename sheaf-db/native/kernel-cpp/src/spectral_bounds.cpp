// spectral_bounds.cpp - C++23 spectral interval and H1 quotient solve (C-02).
//
// This file owns the arithmetic that NF-02 and NF-04 placed in Rust. The Rust
// tier keeps the orchestration and the public API. The C++ tier owns the inner
// loops. All functions use the C ABI declared in sheaf_kernel.h.
//
// Contract: every output pointer must be non-null. A bad argument returns -1.
// No function allocates memory on the heap.

#include "internal.hpp"

#include <cmath>
#include <cstdint>
#include <cstring>

namespace sheaf::kernel {
namespace {

// Apply delta: a 0-cochain becomes a 1-cochain. The restriction weights are
// optional. A null pointer means the identity morphism on that endpoint.
void coboundary_apply(const SheafComplexDesc* cx, const float* x, float* edge) {
    const uint32_t d = cx->stalk_dim;
    for (uint32_t e = 0; e < cx->num_edges; ++e) {
        const uint32_t src = cx->edge_src[e];
        const uint32_t dst = cx->edge_dst[e];
        for (uint32_t k = 0; k < d; ++k) {
            const float left = x[src * d + k];
            const float right = x[dst * d + k];
            const float weighted_left =
                cx->r_src ? left * cx->r_src[e * d + k] : left;
            const float weighted_right =
                cx->r_dst ? right * cx->r_dst[e * d + k] : right;
            edge[e * d + k] = weighted_right - weighted_left;
        }
    }
}

// Apply delta adjoint: a 1-cochain becomes a 0-cochain.
void coboundary_adjoint(const SheafComplexDesc* cx, const float* edge, float* out) {
    const uint32_t d = cx->stalk_dim;
    std::memset(out, 0, static_cast<size_t>(cx->num_vertices) * d * sizeof(float));
    for (uint32_t e = 0; e < cx->num_edges; ++e) {
        const uint32_t src = cx->edge_src[e];
        const uint32_t dst = cx->edge_dst[e];
        for (uint32_t k = 0; k < d; ++k) {
            const float value = edge[e * d + k];
            const float left = cx->r_src ? value * cx->r_src[e * d + k] : value;
            const float right = cx->r_dst ? value * cx->r_dst[e * d + k] : value;
            out[src * d + k] -= left;
            out[dst * d + k] += right;
        }
    }
}

// The Laplacian has a constant kernel. Remove the mean over every component.
void center(float* x, uint32_t n) {
    if (n == 0) return;
    double sum = 0.0;
    for (uint32_t i = 0; i < n; ++i) sum += x[i];
    const float mean = static_cast<float>(sum / n);
    for (uint32_t i = 0; i < n; ++i) x[i] -= mean;
}

float dot(const float* a, const float* b, uint32_t n) {
    double sum = 0.0;
    for (uint32_t i = 0; i < n; ++i) sum += static_cast<double>(a[i]) * b[i];
    return static_cast<float>(sum);
}

} // namespace
} // namespace sheaf::kernel

extern "C" int sheaf_spectral_interval(double mu, double lambda_max,
                                       double* lower, double* upper,
                                       double* center_out, double* radius,
                                       double* sigma) {
    if (!lower || !upper || !center_out || !radius || !sigma) return -1;
    if (!std::isfinite(mu) || mu <= 0.0) return -1;
    if (!std::isfinite(lambda_max) || lambda_max <= 0.0) return -1;
    const double hi = 1.05 * lambda_max + mu;
    *lower = mu;
    *upper = hi;
    *center_out = (hi + mu) * 0.5;
    *radius = (hi - mu) * 0.5;
    if (*radius <= 0.0) return -1;
    *sigma = *center_out / *radius;
    return 0;
}

extern "C" double sheaf_chebyshev_t(double sigma, uint32_t degree) {
    if (!std::isfinite(sigma)) return 0.0;
    double previous = 1.0;
    double current = sigma;
    if (degree == 0) return previous;
    if (degree == 1) return current;
    for (uint32_t k = 1; k < degree; ++k) {
        const double next = 2.0 * sigma * current - previous;
        previous = current;
        current = next;
    }
    return current;
}

extern "C" int sheaf_h1_solve(const SheafComplexDesc* cx, const float* b,
                              float tolerance, uint32_t max_iterations,
                              float* x, float* obstruction,
                              float* residual_norm) {
    if (!cx || !b || !x || !obstruction || !residual_norm) return -1;
    if (!std::isfinite(tolerance) || tolerance <= 0.0f || max_iterations == 0) return -1;
    if (cx->stalk_dim == 0 || cx->num_edges == 0) return -1;
    const uint32_t n = cx->num_vertices * cx->stalk_dim;
    const uint32_t m = cx->num_edges * cx->stalk_dim;
    if (n == 0 || m == 0) return -1;

    std::memset(x, 0, static_cast<size_t>(n) * sizeof(float));
    float* rhs = new float[n];
    float* residual = new float[n];
    float* direction = new float[n];
    float* applied = new float[n];
    float* edges = new float[m];

    sheaf::kernel::coboundary_adjoint(cx, b, rhs);
    sheaf::kernel::center(rhs, n);
    std::memcpy(residual, rhs, static_cast<size_t>(n) * sizeof(float));
    std::memcpy(direction, residual, static_cast<size_t>(n) * sizeof(float));
    float norm = sheaf::kernel::dot(residual, residual, n);

    for (uint32_t iteration = 0; iteration < max_iterations; ++iteration) {
        if (std::sqrt(norm) <= tolerance) break;
        sheaf::kernel::coboundary_apply(cx, direction, edges);
        sheaf::kernel::coboundary_adjoint(cx, edges, applied);
        const float denominator = sheaf::kernel::dot(direction, applied, n);
        if (!std::isfinite(denominator) || denominator <= 1.0e-30f) break;
        const float alpha = norm / denominator;
        for (uint32_t i = 0; i < n; ++i) {
            x[i] += alpha * direction[i];
            residual[i] -= alpha * applied[i];
        }
        sheaf::kernel::center(x, n);
        sheaf::kernel::center(residual, n);
        const float next = sheaf::kernel::dot(residual, residual, n);
        const float beta = next / norm;
        for (uint32_t i = 0; i < n; ++i) direction[i] = residual[i] + beta * direction[i];
        norm = next;
    }

    sheaf::kernel::coboundary_apply(cx, x, edges);
    for (uint32_t i = 0; i < m; ++i) obstruction[i] = b[i] - edges[i];
    double squared = 0.0;
    for (uint32_t i = 0; i < m; ++i) squared += static_cast<double>(obstruction[i]) * obstruction[i];
    *residual_norm = static_cast<float>(std::sqrt(squared));

    delete[] edges;
    delete[] applied;
    delete[] direction;
    delete[] residual;
    delete[] rhs;
    return 0;
}
