// validity.cpp - branch-free nullable stalk compaction (spec Pillar 1).
//
// Nullable stalk columns carry a validity bitmask. Hot vector loops must
// not branch on individual null slots. On AVX-512 hardware the compaction
// uses _mm512_maskz_compress_ps, which packs the valid lanes of a 16-wide
// register in one instruction. The scalar path is the portable fallback.
//
// Bit order: word i of mask_bits covers slots 64*i .. 64*i+63, least
// significant bit first. This matches the BCSF validity sidecar layout.

#include "internal.hpp"

namespace sheaf::kernel {
namespace {

// Portable compaction. The branch is data dependent, so this path is only
// used when the CPU lacks AVX-512. Correctness is identical either way.
uint32_t compact_scalar(const float* values, const uint64_t* mask_bits,
                        uint32_t count, float* out) {
    uint32_t kept = 0;
    for (uint32_t i = 0; i < count; ++i) {
        const uint64_t word = mask_bits[i / 64];
        if ((word >> (i % 64)) & 1ULL) out[kept++] = values[i];
    }
    return kept;
}

#if SHEAF_X86 && (SHEAF_TARGET_SIMD_OK >= 2 || defined(__GNUC__) || defined(__clang__))
SHEAF_TARGET_AVX512
uint32_t compact_avx512(const float* values, const uint64_t* mask_bits,
                        uint32_t count, float* out) {
    uint32_t kept = 0;
    uint32_t i = 0;
    // Process full 16-lane groups. Each group reads its 16 validity bits
    // from the covering 64-bit word and compresses in one instruction.
    for (; i + 16 <= count; i += 16) {
        const uint64_t word = mask_bits[i / 64];
        const uint32_t shift = i % 64;
        // A 16-bit field never straddles a word boundary because 16 | 64.
        const __mmask16 mask = static_cast<__mmask16>((word >> shift) & 0xFFFFULL);
        const __m512 v = _mm512_loadu_ps(values + i);
        const __m512 packed = _mm512_maskz_compress_ps(mask, v);
        // Storing all 16 lanes is safe: kept + 16 <= i + 16 <= count, and
        // the caller sized out for count floats. Lanes past kept are garbage
        // that the next group or the tail overwrites or ignores.
        _mm512_storeu_ps(out + kept, packed);
        kept += static_cast<uint32_t>(_mm_popcnt_u32(mask));
    }
    // Tail slots use the scalar rule against the same bit order.
    for (; i < count; ++i) {
        const uint64_t word = mask_bits[i / 64];
        if ((word >> (i % 64)) & 1ULL) out[kept++] = values[i];
    }
    return kept;
}
#endif

}  // namespace
}  // namespace sheaf::kernel

extern "C" uint32_t sheaf_compact_valid(const float* values,
                                        const uint64_t* mask_bits,
                                        uint32_t count, float* out) {
    if (values == nullptr || mask_bits == nullptr || out == nullptr || count == 0)
        return 0;
#if SHEAF_X86 && (SHEAF_TARGET_SIMD_OK >= 2 || defined(__GNUC__) || defined(__clang__))
    if (sheaf::kernel::has_avx512())
        return sheaf::kernel::compact_avx512(values, mask_bits, count, out);
#endif
    return sheaf::kernel::compact_scalar(values, mask_bits, count, out);
}