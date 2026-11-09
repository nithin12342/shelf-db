// crates/sheaf-engine/src/bin/range_slice_bench.rs — Overhaul D (range gap)
// Gap: ScyllaDB range-slice 65-91ms uncontested (no ordered index existed).
// Fix: NameIndex::range interval API over the sorted key projection.
// Shape: 500k KV cells (key u64 in discrete[8..16], payload f32 stalk),
// 10k-key slice [250000, 260000), sum payloads. Index build untimed
// (reported, same convention as pre-fold); slice timed x5.
// Gate: best < 91ms (Scylla worst); stretch < 65ms (Scylla best).
use std::time::Instant;

use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::bcsf::name_lookup::NameIndex;

const N: u64 = 500_000;
const LO: u64 = 250_000;
const HI: u64 = 260_000;
const TARGET_MS: f64 = 91.0;
const STRETCH_MS: f64 = 65.0;

fn main() {
    // ---- Load (untimed, reported) ----
    let t_build = Instant::now();
    let mut builder = BcsfBuilder::new();
    for key in 1..=N {
        let mut db = [0u8; 16];
        db[..8].copy_from_slice(&1u64.to_le_bytes());
        db[8..].copy_from_slice(&key.to_be_bytes()); // BE: order-preserving for byte-wise index sort
        let payload = (key % 1000) as f32 * 1.5;
        builder.push(0, Vec::new(), &db, &[payload], &[]);
    }
    let reader = builder.finish();
    assert!(reader.validate().is_ok());
    assert_eq!(reader.len() as u64, N);
    let t_idx = Instant::now();
    let index = NameIndex::build(&reader, &[("key".to_string(), 8)], &[(1u64, "kv".to_string())]);
    let idx_ms = t_idx.elapsed().as_secs_f64() * 1e3;
    println!("range_load: cells={} load_s={:.1} index_ms={:.0} resident_MB={:.1}",
        N, t_build.elapsed().as_secs_f64(), idx_ms, reader.resident_bytes() as f64 / 1e6);

    // ---- Exactness baseline: full-scan filter ----
    let mut base = 0f64;
    let mut base_n = 0usize;
    for cell in 0..reader.len() {
        let kb = &reader.discrete(cell)[8..16];
        let k = u64::from_be_bytes(kb.try_into().unwrap());
        if (LO..HI).contains(&k) {
            base += reader.stalk(cell)[0] as f64;
            base_n += 1;
        }
    }
    assert_eq!(base_n, (HI - LO) as usize, "slice width");

    // ---- Timed: interval slice x5 ----
    let cells = index.field_cells("key").expect("key field");
    let mut reps = Vec::new();
    let mut sum = 0f64;
    let mut n = 0usize;
    for _ in 0..5 {
        let t0 = Instant::now();
        let mut acc = 0f64;
        let mut cnt = 0usize;
        for r in index.range("key", LO, HI) {
            for &pos in &cells[r] {
                acc += reader.stalk(pos as usize)[0] as f64;
                cnt += 1;
            }
        }
        std::hint::black_box(acc);
        reps.push(t0.elapsed().as_secs_f64() * 1e3);
        sum = acc;
        n = cnt;
    }
    reps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rel = ((sum - base) / base).abs();
    println!("range_slice: best_ms={:.1} median_ms={:.1} rows={} sum={:.1} rel={:.2e}",
        reps[0], reps[2], n, sum, rel);
    println!("gate_range_exact={}", if rel < 1e-12 && n == base_n { "PASS" } else { "FAIL" });
    println!("gate_range_target={}",
        if reps[0] < STRETCH_MS { "PASS (beat Scylla best)" }
        else if reps[0] < TARGET_MS { "PASS (beat Scylla worst)" }
        else { "RECORDED (Scylla wins)" });
}
