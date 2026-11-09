// xor_delta.cpp - vectorized sparse XOR 0-cochain delta streams (Pillar 1/8).
//
// Historical state reconstruction is pure XOR arithmetic:
//   State(T) = State(T_head) XOR delta_T XOR delta_{T+1} XOR ...
// XOR is commutative and associative, so packet reordering on the wire can
// never diverge the state. The inner loops use the widest XOR instruction
// the CPU exposes: 64 bytes per instruction on AVX-512, 32 on AVX2.

#include "internal.hpp"

namespace sheaf::kernel {
namespace {

// out ^= src over n bytes. Tails drop to 8-byte words, then single bytes.
void xor_scalar(uint8_t* out, const uint8_t* src, size_t n) {
    size_t i = 0;
    for (; i + 8 <= n; i += 8) {
        uint64_t a, b;
        std::memcpy(&a, out + i, 8);
        std::memcpy(&b, src + i, 8);
        a ^= b;
        std::memcpy(out + i, &a, 8);
    }
    for (; i < n; ++i) out[i] ^= src[i];
}

SHEAF_TARGET_AVX2
void xor_avx2(uint8_t* out, const uint8_t* src, size_t n) {
    size_t i = 0;
    for (; i + 32 <= n; i += 32) {
        const __m256i a = _mm256_loadu_si256(reinterpret_cast<const __m256i*>(out + i));
        const __m256i b = _mm256_loadu_si256(reinterpret_cast<const __m256i*>(src + i));
        _mm256_storeu_si256(reinterpret_cast<__m256i*>(out + i), _mm256_xor_si256(a, b));
    }
    xor_scalar(out + i, src + i, n - i);
}

#if SHEAF_TARGET_SIMD_OK >= 2 || defined(__GNUC__) || defined(__clang__)
SHEAF_TARGET_AVX512
void xor_avx512(uint8_t* out, const uint8_t* src, size_t n) {
    size_t i = 0;
    for (; i + 64 <= n; i += 64) {
        const __m512i a = _mm512_loadu_si512(reinterpret_cast<const __m512i*>(out + i));
        const __m512i b = _mm512_loadu_si512(reinterpret_cast<const __m512i*>(src + i));
        _mm512_storeu_si512(reinterpret_cast<__m512i*>(out + i), _mm512_xor_si512(a, b));
    }
    xor_avx2(out + i, src + i, n - i);
}
#endif

// Dispatched XOR-accumulate used by both exported entry points.
void xor_accum(uint8_t* out, const uint8_t* src, size_t n) {
#if SHEAF_X86 && (SHEAF_TARGET_SIMD_OK >= 2 || defined(__GNUC__) || defined(__clang__))
    if (has_avx512()) { xor_avx512(out, src, n); return; }
#endif
#if SHEAF_X86 && SHEAF_TARGET_SIMD_OK
    if (has_avx2()) { xor_avx2(out, src, n); return; }
#endif
    xor_scalar(out, src, n);
}

}  // namespace
}  // namespace sheaf::kernel

using namespace sheaf::kernel;

// Fold a base state forward (or backward) through a chain of deltas.
// out may alias base. deltas may not alias out.
extern "C" int sheaf_xor_delta_fold(const uint8_t* base,
                                    const uint8_t* const* deltas,
                                    uint32_t num_deltas, size_t len,
                                    uint8_t* out) {
    if (base == nullptr || out == nullptr || (num_deltas > 0 && deltas == nullptr))
        return SHEAF_KERNEL_EINVAL;
    if (out != base) std::memcpy(out, base, len);
    for (uint32_t k = 0; k < num_deltas; ++k) {
        if (deltas[k] == nullptr) return SHEAF_KERNEL_EINVAL;
        xor_accum(out, deltas[k], len);
    }
    return SHEAF_KERNEL_OK;
}

// Encode the exact-byte difference between two states as one XOR cochain.
extern "C" int sheaf_xor_delta_encode(const uint8_t* base, const uint8_t* target,
                                      size_t len, uint8_t* delta_out) {
    if (base == nullptr || target == nullptr || delta_out == nullptr)
        return SHEAF_KERNEL_EINVAL;
    size_t i = 0;
#if SHEAF_X86 && (SHEAF_TARGET_SIMD_OK >= 2 || defined(__GNUC__) || defined(__clang__))
    if (has_avx512()) {
        for (; i + 64 <= len; i += 64) {
            const __m512i a = _mm512_loadu_si512(reinterpret_cast<const __m512i*>(base + i));
            const __m512i b = _mm512_loadu_si512(reinterpret_cast<const __m512i*>(target + i));
            _mm512_storeu_si512(reinterpret_cast<__m512i*>(delta_out + i),
                                _mm512_xor_si512(a, b));
        }
    }
#endif
    for (; i < len; ++i) delta_out[i] = base[i] ^ target[i];
    return SHEAF_KERNEL_OK;
}