// crates/sheaf-storage/src/bin/phase1_bench.rs — Phase-1 E2E benchmark binary
// Builds 1M 0-cells + 200k 4-cells, checks SPEC-007 gates.
// V1: discrete payloads stored + verified. V2: median gate + worst-round and
// cold-stride informational rounds. V3: exhaustive sortedness scan. V5:
// math-invariant block. V7: Send+Sync assertion for Phase-2 readers.
use sheaf_core::morphism::compound::CompoundMorphism;
use sheaf_core::morphism::matrix::WeightMatrix;
use sheaf_core::morphism::projection::project_field;
use sheaf_core::poset::id::CellId;
use sheaf_core::poset::incidence::{is_sorted, virtual_face_count};
use sheaf_core::poset::ordering::can_be_face;
use sheaf_core::stalk::discrete::DiscreteStalk;
use sheaf_kernel::simd::intersection::intersect_count;
use sheaf_storage::bcsf::builder::BcsfBuilder;
use std::hint::black_box;
use std::time::Instant;

const N0: usize = 1_000_000;
const NH: usize = 200_000;

fn assert_send_sync<T: Send + Sync>() {}

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn main() {
    // V7: Phase-2 needs 8 concurrent readers over one immutable view.
    assert_send_sync::<sheaf_storage::bcsf::reader::BcsfReader>();

    // V5: math invariants the design relies on but never executed until now.
    for d in [0u8, 1, 2, 3, 4, 255] {
        for i in [0u64, 1, (1u64 << 56) - 1] {
            let id = CellId::new(d, i);
            assert_eq!(id.dim(), d);
            assert_eq!(id.index(), i);
        }
    }
    assert_eq!(WeightMatrix::project_4_to_2().apply(&[1.0, 2.0, 3.0, 4.0]), [1.0, 2.0]);
    assert!(DiscreteStalk::from_u64(42).eq_bytes(&DiscreteStalk::from_u64(42)));
    assert!(!DiscreteStalk::from_u64(42).eq_bytes(&DiscreteStalk::from_u64(43)));
    assert!(can_be_face(0, 2) && !can_be_face(2, 2) && !can_be_face(3, 2));
    assert_eq!(virtual_face_count(4), 4);
    assert!(is_sorted(&[1, 2, 2, 3]));
    assert!(!is_sorted(&[3, 1]));
    let mut payload = [0u8; 16];
    payload[0] = 0xAB;
    let mut mask = [0u8; 16];
    mask[0] = 0x0F;
    assert_eq!(project_field(&payload, &mask)[0], 0x0B);
    assert_eq!(CompoundMorphism::identity_like().mask, [0xFF; 16]);
    println!("invariants=PASS");

    let t0 = Instant::now();
    let mut b = BcsfBuilder::new();
    let empty_morph: [f32; 0] = [];
    for i in 0..N0 {
        let f = i as f32;
        let stalk = [f * 0.001, -f * 0.0004, f * 0.0009, 0.12];
        b.push(0, Vec::new(), &DiscreteStalk::from_u64(i as u64).bytes, &stalk, &empty_morph);
    }
    let morph = [1.0f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let hstalk = [0.20f32, -0.38];
    for h in 0..NH {
        let base = (h * 4) as u64;
        b.push(
            2,
            vec![base, base + 1, base + 2, base + 3],
            &DiscreteStalk::from_u64(9000 + h as u64).bytes,
            &hstalk,
            &morph,
        );
    }
    let r = b.finish();
    let build_ms = t0.elapsed().as_millis();
    assert_eq!(r.len(), N0 + NH);
    r.validate().expect("BCSF structural validate");
    println!("validate=PASS");

    // Full seed verification: every one of the 1.2M cells is read back and
    // compared against its deterministic derivation. Spot checks prove
    // nothing about the other 1,199,998 cells; this pass does.
    let mut seed_bad = 0usize;
    for c in 0..N0 {
        if r.discrete(c) != DiscreteStalk::from_u64(c as u64).bytes {
            seed_bad += 1;
        }
        let f = c as f32;
        let s = r.stalk(c);
        if s != [f * 0.001, -f * 0.0004, f * 0.0009, 0.12] {
            seed_bad += 1;
        }
        if !r.morph_weights(c).is_empty() {
            seed_bad += 1;
        }
    }
    for h in 0..NH {
        let c = N0 + h;
        let base = (h * 4) as u64;
        if r.boundary(c) != [base, base + 1, base + 2, base + 3] {
            seed_bad += 1;
        }
        if r.discrete(c) != DiscreteStalk::from_u64(9000 + h as u64).bytes {
            seed_bad += 1;
        }
        if r.stalk(c) != [0.20f32, -0.38] {
            seed_bad += 1;
        }
        if r.morph_weights(c) != [1.0f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0] {
            seed_bad += 1;
        }
    }
    assert_eq!(seed_bad, 0, "seed verification failures");
    println!("seed_verify=PASS cells={} mismatches=0", N0 + NH);
    assert_eq!(intersect_count(&[1, 2, 3], &[2, 3, 4]), 2);
    assert_eq!(intersect_count(r.boundary(N0), r.boundary(N0 + 1)), 0);

    // V3: exhaustive sortedness scan over every stored boundary slice.
    let mut entries = 0usize;
    for c in 0..r.len() {
        let sl = r.boundary(c);
        assert!(is_sorted(sl), "unsorted boundary at cell {}", c);
        entries += sl.len();
    }
    assert_eq!(entries, NH * 4);
    println!("sorted_scan=PASS entries={}", entries);

    // V2: pre-generated indices so only boundary() is timed; median of 7
    // rounds gates, worst-round and cold-stride rounds are informational.
    let iters = 200_000usize;
    let mut st = 0x9E3779B97F4A7C15u64;
    let mut idx = Vec::with_capacity(iters);
    for _ in 0..iters {
        idx.push((xorshift(&mut st) as usize) % r.len());
    }
    let mut avgs = [0.0f64; 11];
    let mut acc = 0usize;
    // Warmup: excludes frequency-ramp and page-fault transients (this host
    // aggressively scales clocks). Untimed; only steady state is measured.
    for _ in 0..3 {
        for &cell in &idx {
            let sl = r.boundary(cell);
            acc += black_box(sl.len());
            if let Some(&v) = sl.first() {
                acc = acc.wrapping_add(black_box(v as usize));
            }
        }
    }
    for round in avgs.iter_mut() {
        let t1 = Instant::now();
        for &cell in &idx {
            // True boundary fetch: length PLUS first id forces the load of
            // the indices cache line, not just the offset pair. A len-only
            // sink measures offset access and understates the lookup.
            let sl = r.boundary(cell);
            acc += black_box(sl.len());
            if let Some(&v) = sl.first() {
                acc = acc.wrapping_add(black_box(v as usize));
            }
        }
        *round = t1.elapsed().as_nanos() as f64 / iters as f64;
    }
    avgs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg_ns = avgs[5];
    let worst_ns = avgs[10];
    // Cold round: prime stride defeats prefetch, single pass, informational.
    let t2 = Instant::now();
    let mut cold_acc = 0usize;
    for k in 0..iters {
        let sl = r.boundary((k * 7919) % r.len());
        cold_acc += black_box(sl.len());
        if let Some(&v) = sl.first() {
            cold_acc = cold_acc.wrapping_add(black_box(v as usize));
        }
    }
    let cold_ns = t2.elapsed().as_nanos() as f64 / iters as f64;

    let raw = r.raw_payload_bytes() + r.len();
    let resident = r.resident_bytes();
    let ratio = resident as f64 / raw as f64;
    // Capacity-based ratio: proves the freeze step removed Vec slack, so
    // resident_bytes is the true allocation rather than a len() fiction.
    let cap_ratio = r.resident_capacity_bytes() as f64 / raw as f64;

    println!("cells={} hypercells={}", N0, NH);
    println!("build_ms={}", build_ms);
    println!("raw_payload_bytes={}", r.raw_payload_bytes());
    println!("resident_bytes={}", resident);
    println!("mem_ratio={:.3}", ratio);
    println!("mem_ratio_capacity={:.3}", cap_ratio);
    println!("lookup_iters={}", iters);
    println!("lookup_avg_ns={:.2}", avg_ns);
    println!("lookup_worst_round_ns={:.2}", worst_ns);
    println!("lookup_cold_stride_ns={:.2}", cold_ns);
    println!("acc_sink={}", acc + cold_acc);
    let mem_ok = ratio < 1.5 && cap_ratio < 1.5;
    let look_ok = avg_ns < 15.0;
    println!("gate_mem_1.5x={}", if mem_ok { "PASS" } else { "FAIL" });
    println!("gate_lookup_sub15ns={}", if look_ok { "PASS" } else { "FAIL" });
    if !(mem_ok && look_ok) {
        std::process::exit(1);
    }
}
