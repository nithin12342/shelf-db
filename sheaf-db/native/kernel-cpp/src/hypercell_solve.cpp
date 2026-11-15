// Variable-stalk hypercell solve. The caller owns all memory.
// The compressed sparse row (CSR) matrix is delta, never delta^T delta.
#include "internal.hpp"
#include <algorithm>
#include <limits>

namespace {
struct Csr {
    size_t cols, rows, nnz;
    const size_t* offsets;
    const size_t* columns;
    const float* weights;

    // Fuse delta with its adjoint, without a row-sized temporary.
    void apply(const double* x, double lambda, double* out) const {
        for (size_t i = 0; i < cols; ++i) out[i] = lambda * x[i];
        for (size_t row = 0; row < rows; ++row) {
            double v = 0.0;
            for (size_t k = offsets[row]; k < offsets[row + 1]; ++k)
                v += static_cast<double>(weights[k]) * x[columns[k]];
            for (size_t k = offsets[row]; k < offsets[row + 1]; ++k)
                out[columns[k]] += static_cast<double>(weights[k]) * v;
        }
    }

    double energy(const double* x) const {
        double sum = 0.0;
        for (size_t row = 0; row < rows; ++row) {
            double v = 0.0;
            for (size_t k = offsets[row]; k < offsets[row + 1]; ++k)
                v += static_cast<double>(weights[k]) * x[columns[k]];
            sum += v * v;
        }
        return sum;
    }
};

double dot(const double* a, const double* b, size_t n) {
    double sum = 0.0;
    for (size_t i = 0; i < n; ++i) sum += a[i] * b[i];
    return sum;
}
} // namespace

// Return 0 for a finite result, 1 for invalid arguments, 2 for numeric overflow.
// Exhaustion is conveyed through converged, not disguised as convergence.
// Raw callers must provide valid nonoverlapping allocations of the stated sizes.
extern "C" int32_t sheaf_hypercell_solve(
    size_t cols, size_t rows, size_t nnz, const size_t* offsets,
    const size_t* columns, const float* weights, const float* initial,
    float regularization, size_t max_iters, float tolerance,
    float* output, float* energy, size_t* iterations, int32_t* converged,
    double* scratch, size_t scratch_len) noexcept {
    if (!cols || cols > 262144 || rows > 262144 || nnz > 1048576 ||
        !max_iters || max_iters > 4096 ||
        !std::isfinite(regularization) || regularization < 0.0f ||
        !std::isfinite(tolerance) || tolerance <= 0.0f ||
        !offsets || !initial || !output || !energy || !iterations ||
        !converged || !scratch || (nnz && (!columns || !weights))) return 1;
    // Bounds above make addition, multiplication, and pointer offsets safe.
    if (scratch_len < 4 * cols ||
        cols + rows + nnz + 1 > 100000000 / max_iters) return 1;
    if (offsets[0] != 0 || offsets[rows] != nnz) return 1;
    for (size_t row = 0; row < rows; ++row)
        if (offsets[row] > offsets[row + 1] || offsets[row + 1] > nnz) return 1;
    for (size_t k = 0; k < nnz; ++k)
        if (columns[k] >= cols || !std::isfinite(weights[k])) return 1;
    for (size_t i = 0; i < cols; ++i)
        if (!std::isfinite(initial[i])) return 1;

    const Csr m{cols, rows, nnz, offsets, columns, weights};
    double* x = scratch;
    double* r = x + cols;
    double* p = r + cols;
    double* ap = p + cols;
    const double lambda = regularization;
    double rhs_squared = 0.0;
    for (size_t i = 0; i < cols; ++i) {
        x[i] = initial[i];
        const double rhs = lambda * initial[i];
        rhs_squared += rhs * rhs;
    }
    const double target = tolerance * std::max(1.0, std::sqrt(rhs_squared));
    m.apply(x, lambda, ap);
    for (size_t i = 0; i < cols; ++i) p[i] = r[i] = lambda * initial[i] - ap[i];
    double rr = dot(r, r, cols);
    *iterations = 0;
    *converged = 0;
    while (std::sqrt(rr) > target && *iterations < max_iters) {
        m.apply(p, lambda, ap);
        const double pap = dot(p, ap, cols);
        if (!std::isfinite(pap) || pap <= 0.0) break;
        const double alpha = rr / pap;
        for (size_t i = 0; i < cols; ++i) {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        ++*iterations;
        double next = dot(r, r, cols);
        if (!std::isfinite(next)) return 2;
        if (std::sqrt(next) <= target) {
            m.apply(x, lambda, ap);
            for (size_t i = 0; i < cols; ++i) p[i] = r[i] = lambda * initial[i] - ap[i];
            next = dot(r, r, cols);
        } else {
            const double beta = next / rr;
            for (size_t i = 0; i < cols; ++i) p[i] = r[i] + beta * p[i];
        }
        rr = next;
    }
    for (size_t i = 0; i < cols; ++i) {
        if (!std::isfinite(x[i]) || std::abs(x[i]) > std::numeric_limits<float>::max()) return 2;
        output[i] = static_cast<float>(x[i]);
        x[i] = output[i];
    }
    // Measure the returned Float32 solution, including its rounding error.
    m.apply(x, lambda, ap);
    for (size_t i = 0; i < cols; ++i) r[i] = lambda * initial[i] - ap[i];
    *converged = std::sqrt(dot(r, r, cols)) <= target ? 1 : 0;
    const double e = m.energy(x);
    if (!std::isfinite(e) || e > std::numeric_limits<float>::max()) return 2;
    *energy = static_cast<float>(e);
    return 0;
}
