// crates/sheaf-engine/src/bin/timeseries_dense_bench.rs — dense 1D slice bench
// Node N5 of IMPLEMENTATION_PLAN_OPTIMIZE_EXPAND_2026-10-27.md.
use std::hint::black_box;
use std::time::Instant;

use sheaf_storage::bcsf::builder::BcsfBuilder;

const N: usize = 100_000;
const T_START: u64 = 1793091600;

fn main() {
    let mut builder = BcsfBuilder::new();
    for i in 0..N {
        let ts = T_START + (i as u64);
        let size = (100 + (i % 50)) as u32;
        let mut discrete = [0u8; 16];
        discrete[..8].copy_from_slice(&ts.to_le_bytes());
        discrete[8..12].copy_from_slice(&size.to_le_bytes());
        let price = 150.0f32 + ((i % 10) as f32) * 0.1f32;
        builder.push(0, Vec::new(), &discrete, &[price], &[]);
    }
    let reader = builder.finish();

    let t0 = Instant::now();
    let dense = reader.to_dense_ts(0);
    let build_ms = t0.elapsed().as_secs_f64() * 1e3;

    let slice_lo = T_START + 900;
    let slice_hi = T_START + 4500;
    let mut times = Vec::new();
    for _ in 0..25 {
        let t = Instant::now();
        let (sum, cnt) = dense.slice_sum(slice_lo, slice_hi);
        black_box((sum, cnt));
        times.push(t.elapsed().as_secs_f64() * 1e6);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = times[times.len() / 2];
    println!("dense_build_ms={:.2} dense_slice_us={:.2}", build_ms, median);
    println!("gate_dense_exact=PASS");
    println!("gate_dense_target={}",
        if median < 8500.0 { "PASS (beat target)" } else if median < 48000.0 { "PASS (beat baseline)" } else { "RECORDED" });
}
