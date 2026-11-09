// cpu_features.cpp - runtime instruction-set probe for the kernel tier.
//
// The probe runs once. The result is cached in an atomic word so worker
// threads on different cores read the same capability set without locks.
// CPUID leaf 7 reports the AVX2 and AVX-512 feature bits on x86-64. ARM
// builds report NEON, which every AArch64 CPU implements.

#include "internal.hpp"

#if SHEAF_X86
#if defined(_MSC_VER)
#include <intrin.h>
static void cpuid_regs(int leaf, int subleaf, int out[4]) {
    __cpuidex(out, leaf, subleaf);
}
#else
#include <cpuid.h>
static void cpuid_regs(int leaf, int subleaf, int out[4]) {
    __cpuid_count(leaf, subleaf, out[0], out[1], out[2], out[3]);
}
#endif
#endif

namespace sheaf::kernel {

// Fill the capability word from the hardware, not from compile flags.
// A binary built with AVX-512 support still runs on an AVX2-only CPU
// because every dispatch wrapper checks this word first.
//
// _xgetbv needs the xsave target feature on GNU and clang. The
// attribute below enables it for this function only.
#if defined(__GNUC__) || defined(__clang__)
__attribute__((target("xsave")))
#endif
uint32_t probe_caps() {
    uint32_t bits = 0;
#if SHEAF_X86
    int regs[4] = {0, 0, 0, 0};
    cpuid_regs(0, 0, regs);
    const int max_leaf = regs[0];
    if (max_leaf >= 1) {
        cpuid_regs(1, 0, regs);
        // ECX bit 27 = OSXSAVE, bit 28 = AVX. Both must hold before XGETBV.
        const bool osxsave = (regs[2] & (1 << 27)) != 0;
        const bool avx = (regs[2] & (1 << 28)) != 0;
        if (osxsave && avx) {
            // XCR0 bits 1 and 2 must be set: the OS saves XMM and YMM state.
            const uint64_t xcr0 = _xgetbv(0);
            const bool ymm_saved = (xcr0 & 0x6) == 0x6;
            if (ymm_saved && max_leaf >= 7) {
                cpuid_regs(7, 0, regs);
                if (regs[1] & (1 << 5)) bits |= SHEAF_CPU_AVX2;
                // AVX-512F plus the BW/VL subsets the kernels actually use.
                // XCR0 bits 5..7 must also be set for ZMM and opmask state.
                const bool zmm_saved = (xcr0 & 0xE0) == 0xE0;
                const bool f = (regs[1] & (1 << 16)) != 0;
                const bool bw = (regs[1] & (1 << 30)) != 0;
                const bool vl = (regs[1] & (1u << 31)) != 0;
                if (zmm_saved && f && bw && vl) bits |= SHEAF_CPU_AVX512F;
            }
        }
    }
#elif defined(__aarch64__) || defined(_M_ARM64)
    bits |= SHEAF_CPU_NEON;
#endif
    int expected = -1;
    caps_storage().compare_exchange_strong(expected, static_cast<int>(bits),
                                           std::memory_order_relaxed);
    return bits;
}

}  // namespace sheaf::kernel

extern "C" uint32_t sheaf_kernel_cpu_features(void) {
    return sheaf::kernel::caps();
}

extern "C" uint32_t sheaf_kernel_abi_version(void) { return 1; }