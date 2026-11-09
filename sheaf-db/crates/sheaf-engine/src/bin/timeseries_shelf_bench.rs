// crates/sheaf-engine/src/bin/timeseries_shelf_bench.rs — ShelfDB time-series benchmark
// Measures 100,000 tick rows matching Kdb+ and DuckDB time-series benchmarks.
use std::hint::black_box;
use std::time::Instant;

use sheaf_storage::bcsf::builder::BcsfBuilder;

const N: usize = 100_000;
const T_START: u64 = 1793091600; // 2026-10-27 09:00:00 UTC in seconds

fn main() {
    // 1. Build BCSF buffer with 100,000 ticks
    let mut builder = BcsfBuilder::new();
    for i in 0..N {
        let ts = T_START + (i as u64);
        let size = (100 + (i % 50)) as u32;
        let sym_id = (i % 3) as u32;
        let mut discrete = [0u8; 16];
        discrete[..8].copy_from_slice(&ts.to_le_bytes());
        discrete[8..12].copy_from_slice(&size.to_le_bytes());
        discrete[12..16].copy_from_slice(&sym_id.to_le_bytes());
        let price = 150.0f32 + ((i % 10) as f32) * 0.1f32;
        builder.push(0, Vec::new(), &discrete, &[price], &[]);
    }
    let reader = builder.finish();
    assert_eq!(reader.len(), N);

    // Warmup
    let mut dummy = 0.0f64;
    for c in 0..reader.len() {
        dummy += reader.stalk(c)[0] as f64;
    }
    black_box(dummy);

    // Query 1: 1-minute bucket aggregation (time / 60)
    let mut bucket_times = Vec::new();
    for _ in 0..25 {
        let t0 = Instant::now();
        let mut bucket_sum = [0.0f64; 1667];
        let mut bucket_cnt = [0u32; 1667];
        let mut bucket_size = [0u64; 1667];
        for c in 0..reader.len() {
            let disc = reader.discrete(c);
            let ts = u64::from_le_bytes(disc[..8].try_into().unwrap());
            let sz = u32::from_le_bytes(disc[8..12].try_into().unwrap());
            let p = reader.stalk(c)[0] as f64;
            let b = ((ts - T_START) / 60) as usize;
            bucket_sum[b] += p;
            bucket_cnt[b] += 1;
            bucket_size[b] += sz as u64;
        }
        black_box((bucket_sum, bucket_cnt, bucket_size));
        bucket_times.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    bucket_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_bucket = bucket_times[bucket_times.len() / 2];

    // Query 2: Range slice filter (1-hour slice = 3600 seconds)
    let slice_lo = T_START + 900;
    let slice_hi = T_START + 4500;
    let mut slice_times = Vec::new();
    for _ in 0..25 {
        let t0 = Instant::now();
        let mut sum_p = 0.0f64;
        let mut sum_sz = 0u64;
        let mut cnt = 0usize;
        for c in 0..reader.len() {
            let disc = reader.discrete(c);
            let ts = u64::from_le_bytes(disc[..8].try_into().unwrap());
            if ts >= slice_lo && ts <= slice_hi {
                let sz = u32::from_le_bytes(disc[8..12].try_into().unwrap());
                let p = reader.stalk(c)[0] as f64;
                sum_p += p;
                sum_sz += sz as u64;
                cnt += 1;
            }
        }
        black_box((sum_p, sum_sz, cnt));
        slice_times.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    slice_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_slice = slice_times[slice_times.len() / 2];

    // Query 3: Exact point lookup
    let target_ts = T_START + 2535;
    let mut point_times = Vec::new();
    for _ in 0..25 {
        let t0 = Instant::now();
        let mut found_price = 0.0f32;
        let mut found_size = 0u32;
        // In BCSF index, sorted timestamp yields binary search
        let mut low = 0usize;
        let mut high = reader.len();
        while low < high {
            let mid = (low + high) / 2;
            let disc = reader.discrete(mid);
            let ts = u64::from_le_bytes(disc[..8].try_into().unwrap());
            if ts == target_ts {
                found_price = reader.stalk(mid)[0];
                found_size = u32::from_le_bytes(disc[8..12].try_into().unwrap());
                break;
            } else if ts < target_ts {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        black_box((found_price, found_size));
        point_times.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    point_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_point = point_times[point_times.len() / 2];

    // Query 4: Full column scan average (4-accumulator loop)
    let mut scan_times = Vec::new();
    for _ in 0..25 {
        let t0 = Instant::now();
        let mut acc0 = 0.0f64;
        let mut acc1 = 0.0f64;
        let mut acc2 = 0.0f64;
        let mut acc3 = 0.0f64;
        let len = reader.len();
        let chunks = len / 4;
        for i in 0..chunks {
            let base = i * 4;
            acc0 += reader.stalk(base)[0] as f64;
            acc1 += reader.stalk(base + 1)[0] as f64;
            acc2 += reader.stalk(base + 2)[0] as f64;
            acc3 += reader.stalk(base + 3)[0] as f64;
        }
        let total = acc0 + acc1 + acc2 + acc3;
        let avg = total / (len as f64);
        black_box(avg);
        scan_times.push(t0.elapsed().as_secs_f64() * 1e6);
    }
    scan_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_scan = scan_times[scan_times.len() / 2];

    println!("=== SHELFDB BCSF TIME-SERIES BENCHMARK RESULTS (100,000 ROWS) ===");
    println!("window_agg_us: {:.2}", median_bucket);
    println!("slice_filter_us: {:.2}", median_slice);
    println!("point_lookup_us: {:.2}", median_point);
    println!("column_avg_us: {:.2}", median_scan);
}
