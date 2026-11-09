//! Shared memory-hierarchy measurement core for the physical-limits probe.
//! Bin-local module (NOT part of the library) - lives under src/bin/shared/
//! so cargo never builds it as a target or links it into the library.
//! Extracted from physical_limits_probe.rs to satisfy SPEC-022 (<=500 lines/file).
//!
//! Provides the primitives the probe needs:
//!   1. `chase_latency_ns`             - dependent-load latency (ns/load)
//!   2. `chase_latency_page_spread_ns` - latency with independent control
//!      over the cache footprint and the TLB footprint
//!   3. `read_bandwidth_gbs`           - streaming read bandwidth
//!   4. `measure_clock_ghz`            - effective clock, cross-checked
//!   5. `flops_*`                     - compute rate (cache-resident)
//!
//! METHODOLOGY NOTE (why the chain lives in the buffer):
//! A pointer chase is only a MEMORY measurement if the loaded value
//! produces the NEXT load's address. The first version of this code kept
//! the permutation in a side array and read the payload separately, so the
//! dependency chain lived in a small resident array and the measurement
//! reported CACHE latency while claiming to report DRAM latency. Every
//! chase here stores the next address INSIDE the buffer.

use std::time::Instant;

/// xorshift64* - deterministic PRNG so permutations are reproducible.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// Dependent-load latency (ns per load) over a working set of `bytes`.
///
/// A random permutation is walked as a linked list: every load's address is
/// produced by the previous load, so the CPU cannot overlap them. The
/// measured time is therefore the true serial latency of the level that
/// holds the set, not bandwidth.
///
/// BEST-OF-N: the probe repeats each measurement and keeps the MINIMUM.
/// On a shared, thermally-governed laptop the minimum is the least
/// contended estimate of the hardware latency; the median is contaminated
/// by whatever else the box is running. This is the same precedent the
/// repo already uses for the Phase-1 lookup gate ("run binary 3x, take
/// best").
pub fn chase_latency_ns(bytes: usize, iters: u64) -> f64 {
    chase_latency_ns_best_of(bytes, iters, 3)
}

/// Best-of-N dependent-load latency (ns per load) over a working set.
pub fn chase_latency_ns_best_of(bytes: usize, iters: u64, reps: usize) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..reps.max(1) {
        let ns = chase_latency_once(bytes, iters);
        if ns < best {
            best = ns;
        }
    }
    best
}

/// One dependent-load latency measurement (ns per load).
fn chase_latency_once(bytes: usize, iters: u64) -> f64 {
    let n = (bytes / 8).max(16);
    let mut next: Vec<u64> = (0..n as u64).collect();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for i in (1..n).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        next.swap(i, j);
    }
    // Warm: touch the whole set so TLB/DRAM pages are resident.
    let mut idx = 0usize;
    for _ in 0..(n / 4).max(1) {
        idx = next[idx] as usize;
    }
    let t0 = Instant::now();
    for _ in 0..iters {
        idx = next[idx] as usize;
    }
    let elapsed = t0.elapsed().as_secs_f64();
    std::hint::black_box(idx);
    elapsed / iters as f64 * 1e9
}

/// Random-access latency for a controlled (footprint, page-count) pair.
///
/// `data_bytes` of distinct data is placed in `pages` groups of
/// `data_bytes/pages` bytes, the groups spread evenly across `span_bytes`.
/// When `data_bytes/pages` is exactly 4096, each group is one 4 KiB page
/// and the groups are separated by `span_bytes/pages`, so the access
/// pattern touches exactly `pages` distinct 4 KiB pages.
///
/// This gives independent control over:
///
/// - the CACHE footprint (how many bytes are touched)
/// - the TLB footprint  (how many distinct pages are touched)
///
/// The chain is stored INSIDE the buffer: each node holds the byte offset
/// of the next node, so the load produces the next address and the load
/// is genuinely in the dependency chain.
pub fn chase_latency_page_spread_ns(
    span_bytes: usize,
    data_bytes: usize,
    pages: usize,
    iters: u64,
) -> f64 {
    const LINE: usize = 64; // one cache line per node
    let total_nodes = data_bytes / LINE;
    assert!(
        total_nodes.is_multiple_of(pages),
        "pages must divide the node count"
    );
    let nodes_per_page = total_nodes / pages;
    let stride = span_bytes / pages;

    // Node index -> byte offset: page p starts at p*stride, and the nodes
    // of a page occupy its first nodes_per_page*LINE bytes.
    let offset_of = |node: usize| -> usize {
        let page = node / nodes_per_page;
        let slot = node % nodes_per_page;
        page * stride + slot * LINE
    };

    // Random permutation over the node indices.
    let mut order: Vec<usize> = (0..total_nodes).collect();
    let mut rng = Rng(0x1234_5678_9ABC_DEF0);
    for i in (1..total_nodes).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        order.swap(i, j);
    }

    // Store the next node's byte offset IN the buffer, so the chase is
    // off = *(u64*)(buf + off) - the load feeds the chain.
    let mut buf = vec![0u8; span_bytes];
    for w in order.windows(2) {
        let at = offset_of(w[0]);
        let next_at = offset_of(w[1]);
        buf[at..at + 8].copy_from_slice(&(next_at as u64).to_le_bytes());
    }
    let at = offset_of(order[total_nodes - 1]);
    let first = offset_of(order[0]);
    buf[at..at + 8].copy_from_slice(&(first as u64).to_le_bytes());

    // Warm: touch every page so the TLB and DRAM pages are resident.
    let mut off = first;
    for _ in 0..total_nodes {
        off = u64::from_le_bytes(buf[off..off + 8].try_into().unwrap()) as usize;
    }

    let t0 = Instant::now();
    for _ in 0..iters {
        off = u64::from_le_bytes(buf[off..off + 8].try_into().unwrap()) as usize;
    }
    let elapsed = t0.elapsed().as_secs_f64();
    std::hint::black_box(off);
    elapsed / iters as f64 * 1e9
}

/// Latency of a random access in the (cache tier x TLB state) quadrant.
///
/// `data_bytes` = cache footprint, `pages` = distinct 4 KiB pages touched.
/// The span is generous so the pages are physically separated (no TLB
/// sharing between adjacent groups).
///
/// BEST-OF-N, for the same contention reason as `chase_latency_ns`.
pub fn quadrant_l3_tlbhit_ns() -> f64 {
    quadrant_best_of(64 << 20, 1024 * 4096, 1024)
}
pub fn quadrant_l3_tlbmiss_ns() -> f64 {
    quadrant_best_of(256 << 20, 2048 * 4096, 2048)
}
pub fn quadrant_dram_tlbmiss_ns() -> f64 {
    quadrant_best_of(512 << 20, 8192 * 4096, 8192)
}

/// Best-of-N latency for one controlled (span, data, pages) configuration.
fn quadrant_best_of(span: usize, data: usize, pages: usize) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..3 {
        let ns = chase_latency_page_spread_ns(span, data, pages, 3_000_000);
        if ns < best {
            best = ns;
        }
    }
    best
}

/// Streaming read bandwidth (GB/s) over `buf` using `threads` threads.
pub fn read_bandwidth_gbs(buf: &[u64], threads: usize) -> f64 {
    let chunk = buf.len() / threads;
    let t0 = Instant::now();
    let mut partials = vec![0u64; threads];
    std::thread::scope(|s| {
        for (t, slot) in partials.iter_mut().enumerate() {
            let part = &buf[t * chunk..(t + 1) * chunk];
            s.spawn(move || {
                let mut acc = 0u64;
                for v in part {
                    acc = acc.wrapping_add(*v);
                }
                *slot = acc;
            });
        }
    });
    let elapsed = t0.elapsed().as_secs_f64();
    std::hint::black_box(&partials);
    (buf.len() * 8) as f64 / elapsed / 1e9
}

/// Effective clock (GHz), derived by MARGINAL COST and cross-checked twice.
///
/// The clock is derived from the MARGINAL cost of adding one more
/// DEPENDENT `add r,1` to a serial chain. `add r64, imm8` has 1-cycle
/// latency on every x86 CPU, so:
///
///     clock = 1 / (seconds per extra dependency)
///
/// Measuring the marginal cost (rather than the total time of a fixed
/// chain) is what makes this sound: any FIXED loop overhead - the counter
/// decrement, the branch, the loop-carried counter dependency - cancels in
/// the difference between the N-add and the N+1-add chain. The remaining
/// assumption is a single one, and it is the one thing every x86
/// reference agrees on: `add r,imm` latency = 1 cycle.
///
/// This matters because the clock is the anchor for every "FLOP/cycle"
/// claim in the report, and the first two versions of this probe got the
/// anchor WRONG:
///
///   v1: a Rust loop, cycle count hand-analysed from the disassembly as
///       "3 ops = 3 cycles". Wrong: LLVM had strength-reduced `x+1`
///       then `x*K` into `imul` + `add K` and unrolled the loop 4x.
///   v2: inline asm, but the chain included a `mov` that is NOT
///       eliminated (it feeds a later `xor`), so the count was 7, not 6.
///
/// The cross-checks below make that class of error impossible:
///
///   CROSS-CHECK A (relative): an 8-add chain vs a 4-add chain must have
///     a cycle ratio of exactly 2.0.
///   CROSS-CHECK B (absolute): a 4 x `imul r,r,imm` chain vs the 4-add
///     chain. `imul r64, r64, imm32` (the 3-operand immediate form used
///     here) has 4-cycle latency on Intel cores of this class, while the
///     2-operand register form `imul r64, r64` has 3 cycles. The chain
///     therefore costs 4 x 4 = 16 cycles against the add chain's 4, so
///     the expected ratio is 4.0. This validates the ABSOLUTE cycle
///     count: if the add chain were really 1.5 cycles the ratio would be
///     2.67, and if it were 0.75 cycles the ratio would be 5.33.
///
/// AGGREGATION (why the median of per-pair ratios, not the ratio of
/// medians). Two earlier designs were wrong and are recorded here so
/// they are not reintroduced:
///
///   - Taking an independent "best of" for each chain and THEN dividing
///     mixes thermal windows: the best add8 came from a cold rep and the
///     best add4 from a hot one, and the ratio of two windows is not a
///     cycle ratio. (This produced 1.784 instead of 2.000.)
///   - Taking one "best of" per chain over a LONG measurement lets the
///     clock drift inside the chain set, which moved the ratio by 30%.
///
/// Instead: many short reps, each measuring the two chains ADJACENTLY
/// (so the window between them is milliseconds), and the MEDIAN of the
/// per-rep ratios. The median is robust to the outliers a
/// thermally-governed laptop produces.
///
/// Returns (clock_GHz, crosscheck_add_ratio, crosscheck_imul_ratio).
#[cfg(target_arch = "x86_64")]
pub fn measure_clock_ghz() -> (f64, f64, f64) {
    use std::arch::asm;
    use std::time::Instant;

    const REPS: usize = 15;
    const PAIR_ITERS: u64 = 4_000_000;

    // --- chain of 4 dependent adds (4 cycles) ---
    let add4_pair = |n: u64| -> u64 {
        let mut x: u64 = 1;
        let c: u64 = n;
        unsafe {
            asm!(
                "2:",
                "add {x}, 1",
                "add {x}, 1",
                "add {x}, 1",
                "add {x}, 1",
                "dec {c}",
                "jnz 2b",
                x = inout(reg) x => x,
                c = inout(reg) c => _,
                options(nostack),
            );
        }
        x
    };
    // --- chain of 8 dependent adds (8 cycles) ---
    let add8_pair = |n: u64| -> u64 {
        let mut x: u64 = 1;
        let c: u64 = n;
        unsafe {
            asm!(
                "2:",
                "add {x}, 1",
                "add {x}, 1",
                "add {x}, 1",
                "add {x}, 1",
                "add {x}, 1",
                "add {x}, 1",
                "add {x}, 1",
                "add {x}, 1",
                "dec {c}",
                "jnz 2b",
                x = inout(reg) x => x,
                c = inout(reg) c => _,
                options(nostack),
            );
        }
        x
    };
    // --- chain of 4 dependent imul, 3-operand immediate form (16 cycles) ---
    let imul4_pair = |n: u64| -> u64 {
        let mut x: u64 = 0x1234_5678_9ABC_DEF0;
        let c: u64 = n;
        unsafe {
            asm!(
                "2:",
                "imul {x}, {x}, 3",
                "imul {x}, {x}, 3",
                "imul {x}, {x}, 3",
                "imul {x}, {x}, 3",
                "dec {c}",
                "jnz 2b",
                x = inout(reg) x => x,
                c = inout(reg) c => _,
                options(nostack),
            );
        }
        x
    };

    let mut ratios_add: Vec<f64> = Vec::with_capacity(REPS);
    let mut ratios_imul: Vec<f64> = Vec::with_capacity(REPS);
    let mut clocks: Vec<f64> = Vec::with_capacity(REPS);

    for _ in 0..REPS {
        // Pair A: 4-add chain then 8-add chain (adjacent).
        let t0 = Instant::now();
        std::hint::black_box(add4_pair(PAIR_ITERS));
        let s4 = t0.elapsed().as_secs_f64();
        let t0 = Instant::now();
        std::hint::black_box(add8_pair(PAIR_ITERS));
        let s8 = t0.elapsed().as_secs_f64();
        ratios_add.push(s8 / s4);
        let marginal = (s8 - s4) / (4.0 * PAIR_ITERS as f64);
        if marginal > 0.0 {
            clocks.push(1.0 / marginal / 1e9);
        }

        // Pair B: 4-add chain then 4-imul chain (adjacent).
        let t0 = Instant::now();
        std::hint::black_box(add4_pair(PAIR_ITERS));
        let s4b = t0.elapsed().as_secs_f64();
        let t0 = Instant::now();
        std::hint::black_box(imul4_pair(PAIR_ITERS));
        let si = t0.elapsed().as_secs_f64();
        ratios_imul.push(si / s4b);
    }

    let median = |mut v: Vec<f64>| -> f64 {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    let clock_ghz = median(clocks);
    let add_ratio = median(ratios_add.clone());
    let imul_ratio = median(ratios_imul.clone());

    println!(
        "C_clock_marginal_s_per_add={:.5e} (const derived: 1 cycle per added dependency)",
        1.0 / (clock_ghz * 1e9)
    );
    println!("C_clock_crosscheck_reps_add={:?}", ratios_add);
    println!("C_clock_crosscheck_reps_imul={:?}", ratios_imul);
    (clock_ghz, add_ratio, imul_ratio)
}

/// Portable fallback: no inline asm, so the clock cannot be derived and
/// the cross-checks are reported as unavailable.
#[cfg(not(target_arch = "x86_64"))]
pub fn measure_clock_ghz() -> (f64, f64, f64) {
    let mut x = 1u64;
    let t0 = std::time::Instant::now();
    for _ in 0..20_000_000u64 {
        x = x.wrapping_add(1);
    }
    let secs = t0.elapsed().as_secs_f64();
    std::hint::black_box(x);
    (20_000_000.0 / secs / 1e9, 0.0, 0.0)
}

/// Number of independent accumulator chains. Must exceed the FMA latency in
/// cycles so the FMA pipes stay full; otherwise we measure FMA latency.
pub const CHAINS: usize = 8;

/// Scalar (SSE) f32 FMA throughput, FLOP count for `iters` passes.
pub fn flops_scalar(a: &[f32], b: &[f32], c: &[f32], iters: usize) -> f64 {
    let mut acc = [1.0f32; CHAINS];
    for _ in 0..iters {
        for (i, v) in a.iter().enumerate() {
            let k = i % CHAINS;
            acc[k] += *v * b[i];
            acc[k] = acc[k] * c[i] + 0.5;
        }
    }
    std::hint::black_box(acc);
    a.len() as f64 * iters as f64 * 2.0
}

#[cfg(target_arch = "x86_64")]
pub mod simd {
    use super::CHAINS;
    use std::arch::x86_64::*;

    /// AVX2 f32 FMA throughput, FLOP count for `iters` passes.
    #[inline(never)]
    #[target_feature(enable = "avx2,fma")]
    pub unsafe fn flops_avx2(a: &[f32], b: &[f32], c: &[f32], iters: usize) -> f64 {
        let n = a.len();
        let mut acc = [_mm256_set1_ps(1.0f32); CHAINS];
        let half = _mm256_set1_ps(0.5f32);
        for _ in 0..iters {
            for i in (0..n).step_by(8 * CHAINS) {
                for (k, slot) in acc.iter_mut().enumerate() {
                    let o = i + k * 8;
                    let va = _mm256_loadu_ps(a.as_ptr().add(o));
                    let vb = _mm256_loadu_ps(b.as_ptr().add(o));
                    let vc = _mm256_loadu_ps(c.as_ptr().add(o));
                    *slot = _mm256_fmadd_ps(va, vb, *slot);
                    *slot = _mm256_fmadd_ps(*slot, vc, half);
                }
            }
        }
        let mut s = _mm256_setzero_ps();
        for v in acc {
            s = _mm256_add_ps(s, v);
        }
        std::hint::black_box(s);
        n as f64 * iters as f64 * 4.0 // 2 FMA x 2 flops each
    }

    /// AVX-512 f32 FMA throughput, FLOP count for `iters` passes.
    #[inline(never)]
    #[target_feature(enable = "avx512f")]
    pub unsafe fn flops_avx512(a: &[f32], b: &[f32], c: &[f32], iters: usize) -> f64 {
        let n = a.len();
        let mut acc = [_mm512_set1_ps(1.0f32); CHAINS];
        let half = _mm512_set1_ps(0.5f32);
        for _ in 0..iters {
            for i in (0..n).step_by(16 * CHAINS) {
                for (k, slot) in acc.iter_mut().enumerate() {
                    let o = i + k * 16;
                    let va = _mm512_loadu_ps(a.as_ptr().add(o));
                    let vb = _mm512_loadu_ps(b.as_ptr().add(o));
                    let vc = _mm512_loadu_ps(c.as_ptr().add(o));
                    *slot = _mm512_fmadd_ps(va, vb, *slot);
                    *slot = _mm512_fmadd_ps(*slot, vc, half);
                }
            }
        }
        let mut s = _mm512_setzero_ps();
        for v in acc {
            s = _mm512_add_ps(s, v);
        }
        std::hint::black_box(s);
        n as f64 * iters as f64 * 4.0
    }
}
