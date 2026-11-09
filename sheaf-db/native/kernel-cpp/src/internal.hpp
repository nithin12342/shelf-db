// internal.hpp - shared dispatch helpers for the C++23 kernel tier.
//
// Every hot operation ships three bodies: scalar, AVX2, and AVX-512.
// The exported C ABI wrapper selects a body once per call through the
// capability word that cpu_features.cpp fills at first use. The branch is
// perfectly predicted after the first call, so dispatch costs nothing in
// steady state. MSVC builds compile the SIMD bodies only when the global
// /arch flags enable them, because MSVC has no per-function target attribute.

#pragma once

#include "../include/sheaf_kernel.h"

#include <atomic>
#include <cstring>

#if defined(__x86_64__) || defined(_M_X64) || defined(__i386__) || defined(_M_IX86)
#define SHEAF_X86 1
#include <immintrin.h>
#else
#define SHEAF_X86 0
#endif

#if defined(__GNUC__) || defined(__clang__)
#define SHEAF_TARGET_AVX2   __attribute__((target("avx2,fma")))
#define SHEAF_TARGET_AVX512 __attribute__((target("avx512f,avx512bw,avx512vl,fma")))
#define SHEAF_TARGET_SIMD_OK 1
#elif defined(_MSC_VER)
#define SHEAF_TARGET_AVX2
#define SHEAF_TARGET_AVX512
// MSVC gates the SIMD bodies on the global architecture flags instead.
#if defined(__AVX512F__)
#define SHEAF_TARGET_SIMD_OK 2
#elif defined(__AVX2__)
#define SHEAF_TARGET_SIMD_OK 1
#else
#define SHEAF_TARGET_SIMD_OK 0
#endif
#else
#define SHEAF_TARGET_AVX2
#define SHEAF_TARGET_AVX512
#define SHEAF_TARGET_SIMD_OK 0
#endif

namespace sheaf::kernel {

// Capability word. -1 means "not probed yet". cpu_features.cpp owns the probe.
inline std::atomic<int>& caps_storage() {
    static std::atomic<int> caps{-1};
    return caps;
}

// Defined in cpu_features.cpp. Returns the cached capability bitset.
uint32_t probe_caps();

inline uint32_t caps() {
    int c = caps_storage().load(std::memory_order_relaxed);
    if (c < 0) c = static_cast<int>(probe_caps());
    return static_cast<uint32_t>(c);
}

inline bool has_avx512() {
#if SHEAF_X86 && SHEAF_TARGET_SIMD_OK
    return (caps() & SHEAF_CPU_AVX512F) != 0;
#else
    return false;
#endif
}

inline bool has_avx2() {
#if SHEAF_X86 && SHEAF_TARGET_SIMD_OK
    return (caps() & (SHEAF_CPU_AVX2 | SHEAF_CPU_AVX512F)) != 0;
#else
    return false;
#endif
}

// ---- small shared vector helpers -----------------------------------------
// Each helper keeps a scalar tail so callers never pad their buffers.

inline float dot_scalar(const float* a, const float* b, size_t n) {
    float sum = 0.0f;
    for (size_t i = 0; i < n; ++i) sum += a[i] * b[i];
    return sum;
}

SHEAF_TARGET_AVX2
inline float dot_avx2(const float* a, const float* b, size_t n) {
    __m256 acc = _mm256_setzero_ps();
    size_t i = 0;
    for (; i + 8 <= n; i += 8)
        acc = _mm256_fmadd_ps(_mm256_loadu_ps(a + i), _mm256_loadu_ps(b + i), acc);
    alignas(32) float lanes[8];
    _mm256_store_ps(lanes, acc);
    float sum = 0.0f;
    for (int k = 0; k < 8; ++k) sum += lanes[k];
    return sum + dot_scalar(a + i, b + i, n - i);
}

#if SHEAF_TARGET_SIMD_OK >= 2 || defined(__GNUC__) || defined(__clang__)
SHEAF_TARGET_AVX512
inline float dot_avx512(const float* a, const float* b, size_t n) {
    __m512 acc = _mm512_setzero_ps();
    size_t i = 0;
    for (; i + 16 <= n; i += 16)
        acc = _mm512_fmadd_ps(_mm512_loadu_ps(a + i), _mm512_loadu_ps(b + i), acc);
    float sum = _mm512_reduce_add_ps(acc);
    return sum + dot_avx2(a + i, b + i, n - i);
}
#endif

// Dispatched dot product used by the solver and the energy path.
inline float dot(const float* a, const float* b, size_t n) {
#if SHEAF_X86 && (SHEAF_TARGET_SIMD_OK >= 2 || defined(__GNUC__) || defined(__clang__))
    if (has_avx512()) return dot_avx512(a, b, n);
#endif
#if SHEAF_X86 && SHEAF_TARGET_SIMD_OK
    if (has_avx2()) return dot_avx2(a, b, n);
#endif
    return dot_scalar(a, b, n);
}

// out[i] = a[i] + alpha * b[i]  (BLAS axpy), dispatched the same way.
inline void axpy_scalar(float* out, const float* b, float alpha, size_t n) {
    for (size_t i = 0; i < n; ++i) out[i] += alpha * b[i];
}

SHEAF_TARGET_AVX2
inline void axpy_avx2(float* out, const float* b, float alpha, size_t n) {
    const __m256 va = _mm256_set1_ps(alpha);
    size_t i = 0;
    for (; i + 8 <= n; i += 8) {
        __m256 r = _mm256_fmadd_ps(va, _mm256_loadu_ps(b + i), _mm256_loadu_ps(out + i));
        _mm256_storeu_ps(out + i, r);
    }
    axpy_scalar(out + i, b + i, alpha, n - i);
}

inline void axpy(float* out, const float* b, float alpha, size_t n) {
#if SHEAF_X86 && SHEAF_TARGET_SIMD_OK
    if (has_avx2()) { axpy_avx2(out, b, alpha, n); return; }
#endif
    axpy_scalar(out, b, alpha, n);
}

// Zero a float range. memset is valid for IEEE-754 +0.0 on every target.
inline void zero(float* p, size_t n) { std::memset(p, 0, n * sizeof(float)); }

// Validate a descriptor before any pointer dereference in the hot paths.
inline bool desc_ok(const SheafComplexDesc* cx) {
    return cx != nullptr && cx->stalk_dim >= 1 &&
           cx->edge_src != nullptr && cx->edge_dst != nullptr &&
           ((cx->r_src == nullptr) == (cx->r_dst == nullptr));
}

}  // namespace sheaf::kernel