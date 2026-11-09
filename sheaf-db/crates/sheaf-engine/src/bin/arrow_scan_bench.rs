use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use sheaf_storage::bcsf::arrow_layout::ArrowStalkSlab;

/// B6: a counting allocator - the "no memory spike" gate must be measured,
/// not asserted.
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);

struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOC_BYTES.fetch_add(l.size() as u64, Ordering::Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new_size: usize) -> *mut u8 {
        ALLOC_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        System.realloc(p, l, new_size)
    }
}

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

/// Expected row count for the TPC-H lineitem table.
const EXPECTED_ROWS: usize = 6_001_215;
/// Expected revenue sum for the TPC-H lineitem table.
const EXPECTED_SUM: f64 = 218102223881.86;

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lineitem = bench_args::required_path(&args, "--lineitem");

    let t0 = Instant::now();
    let mut slab = ArrowStalkSlab::with_capacity(2, EXPECTED_ROWS + 64);
    let content = std::fs::read(&lineitem).expect("lineitem.tbl missing");
    let mut start = 0usize;
    for i in 0..content.len() {
        if content[i] == b'\n' {
            let line = &content[start..i];
            start = i + 1;
            if line.is_empty() {
                continue;
            }
            let mut fstart = 0usize;
            let mut field = 0usize;
            let mut price = 0f32;
            let mut disc = 0f32;
            for j in 0..line.len() {
                if line[j] == b'|' {
                    if field == 5 {
                        price = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap();
                    } else if field == 6 {
                        disc = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap();
                        break;
                    }
                    field += 1;
                    fstart = j + 1;
                }
            }
            slab.push_row(&[price, disc]);
        }
    }
    println!("arrow_load: rows={} load_s={:.1}", slab.len(), t0.elapsed().as_secs_f64());
    assert_eq!(slab.len(), EXPECTED_ROWS, "row count mismatch");

    // Gate 1: 64B alignment.
    let aligned = slab.base_addr().is_multiple_of(64);
    println!("gate_align_64B={} (base=0x{:x})", if aligned { "PASS" } else { "FAIL" }, slab.base_addr());

    // Gate 2: sum correctness (same order/ops as tpch_scan_bench).
    let sum = slab.fused_revenue_sum(0, 1);
    let rel = ((sum - EXPECTED_SUM) / EXPECTED_SUM).abs();
    println!("gate_sum_exact={} (sum={:.2}, rel_diff={:.2e})",
        if rel < 1e-9 { "PASS" } else { "FAIL" }, sum, rel);

    // Gate 3: scan parity (timed x3).
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t0 = Instant::now();
        std::hint::black_box(slab.fused_revenue_sum(0, 1));
        best = best.min(t0.elapsed().as_secs_f64() * 1e3);
    }
    println!("arrow_scan: best_ms={:.1} (3 reps) vs tpch_scan 7.9ms", best);
    println!("gate_parity={}", if best < 7.9 * 1.1 { "PASS (within 10% of baseline)" } else { "RECORDED (regression vs baseline)" });

    // Gate 4 (Stage 1b): FMA kernel over the slab's columns. B6: with SoA as
    // the primary layout the two operands are CONTIGUOUS slices (no gather,
    // no stride arithmetic) - the kernel reads them directly.
    let col_p = slab.column(0);
    let col_q = slab.column(1);
    let mut fma_best = f64::MAX;
    let mut fma_sum = 0f64;
    for _ in 0..3 {
        let t0 = Instant::now();
        fma_sum = sheaf_kernel::simd::fma_scan::fused_sum_columns(col_p, col_q);
        fma_best = fma_best.min(t0.elapsed().as_secs_f64() * 1e3);
    }
    let fma_rel = ((fma_sum - EXPECTED_SUM) / EXPECTED_SUM).abs();
    println!("fma_scan: best_ms={:.1} sum_rel_diff={:.2e} (tol 1e-6, FMA rounding)", fma_best, fma_rel);
    println!("fma_path={}", sheaf_kernel::simd::fma_scan::path_name());
    println!("gate_fma_correct={}", if fma_rel < 1e-6 { "PASS" } else { "FAIL" });
    println!("gate_fma_target={} (goal <= 2.8ms vs duckdb 6.4ms)",
        if fma_best <= 2.8 { "PASS" } else { "RECORDED" });

    // Gate 5 (SoA follow-up): transpose once (reported), then unit-stride
    // sum over contiguous columns — the layout gather cannot match.
    // B6: the transpose is a VIEW into the slab (SoA primary) - it must not
    // allocate. The allocation counter proves it.
    let alloc_before = ALLOC_BYTES.load(Ordering::Relaxed);
    let t0 = Instant::now();
    let (cp, cq) = slab.transpose_soa(0, 1);
    let transpose_ms = t0.elapsed().as_secs_f64() * 1e3;
    let alloc_bytes = ALLOC_BYTES.load(Ordering::Relaxed) - alloc_before;
    let borrowed = matches!(cp, std::borrow::Cow::Borrowed(_))
        && matches!(cq, std::borrow::Cow::Borrowed(_));
    let spike_bytes = (slab.len() * 2 * 4) as u64;
    println!(
        "soa_transpose: allocated_bytes={alloc_bytes} (old AoS path would copy {spike_bytes}) borrowed={borrowed}"
    );
    println!(
        "gate_soa_no_memory_spike={}",
        if borrowed && alloc_bytes < 4096 { "PASS" } else { "FAIL" }
    );
    // Single timed rep (NOT best-of-3 in-process): any intra-process rep
    // loop lets scalar evolution hoist the invariant sum out of the timed
    // region (observed: 0.0/0.0/27.9ms reps with correct sums). Repeat at the
    // PROCESS level (run binary 3x, take best) — immune to hoisting.
    let t0 = Instant::now();
    let mut acc = 0f64;
    // zip iterators: LLVM eliminates bounds checks (unlike dual indexing
    // cp[i]/cq[i], where cq's check survives and costs ~3x).
    for (p, q) in cp.iter().zip(cq.iter()) {
        acc += *p as f64 * (1.0 - *q as f64);
    }
    let soa_ms = t0.elapsed().as_secs_f64() * 1e3;
    let soa_sum = acc;
    let soa_rel = ((soa_sum - EXPECTED_SUM) / EXPECTED_SUM).abs();
    println!("soa_scan: transpose_ms={:.1} (one-time) rep_ms={:.1} rel={:.2e}", transpose_ms, soa_ms, soa_rel);
    println!("gate_soa={} (goal <= 6.4ms duckdb quiet; repeat binary 3x, take best)",
        if soa_ms <= 6.4 && soa_rel < 1e-9 { "PASS (beat DuckDB)" } else { "RECORDED" });

    if !(aligned && rel < 1e-9) {
        std::process::exit(1);
    }
    println!("STAGE-1A: PASS");

    // P2 gate (exactness only, runs after all timings): null every 7th row,
    // verify exception list + compacted dense sum matches the valid subset.
    for r in (0..slab.len()).step_by(7) {
        slab.set_null(r);
    }
    let exc = slab.null_exceptions();
    let (dp, dq, kept) = slab.compact_valid(0, 1);
    let mut acc = 0f64;
    for (a, b) in dp.iter().zip(dq.iter()) {
        acc += *a as f64 * (1.0 - *b as f64);
    }
    // Reference: the slab's own fused sum skips the same nulls, so exact
    // agreement proves compaction preserves the sum; exc non-empty proves
    // nulls actually existed (partition identity checked below).
    let full = slab.fused_revenue_sum(0, 1);
    let ok = exc.windows(2).all(|w| w[0] < w[1])
        && kept + exc.len() == slab.len()
        && !exc.is_empty()
        && (acc - full).abs() < 1e-6;
    println!("gate_p2_exceptions={} (nulls={}, kept={}, dense_sum={:.2})",
        if ok { "PASS" } else { "FAIL" }, exc.len(), kept, acc);
    if !ok {
        std::process::exit(1);
    }
}
