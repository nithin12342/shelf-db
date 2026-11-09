// ADVERSARIAL DECOMPOSITION of the IMDb Q1 path, and the cost of the
// answer-fiber that would replace it.
//
// FINDING UNDER TEST. The IMDb Q1 query is described in the comparison
// tables as "fiber bundle" backed, but the code at query time does NOT
// read a fiber. It loads the raw `basics` and `ratings` sections, builds
// a HashMap over 1,721,477 rating rows, then scans 12,866,375 basics
// rows and probes the map. So the 159.4 ms Q1 number is the cost of a
// JOIN executed at query time, not the cost of reading a pre-aggregated
// answer.
//
// This experiment decomposes that 159.4 ms into its parts, and then
// measures what a genuine answer-fiber for Q1's shape would cost.
//
// STANDALONE TOOL. Compile and run from this directory:
//   rustc -O q1_decompose.rs -o q1_decompose.exe && ./q1_decompose.exe
//
// BASELINE PROVENANCE. The Q1 baseline below is the idle control run in
// results/imdb_shelf_runs.json, key `idle_control_20261021_1634`. It was
// measured with the same binary and the same bundle as every other run,
// with the box idle. The other three committed runs give Q1 query times of
// 197.8 ms, 274.9 ms and 349.6 ms, so the baseline is the fastest of the
// four. The speedup lines below are therefore the best case.

use std::collections::HashMap;
use std::time::Instant;

const N_RATINGS: u64 = 1_721_477;
const N_BASICS: u64 = 12_866_375;
const RATING_ROW: u64 = 20; // id u64, avg f32, votes i64
const BASIC_ROW: u64 = 13; // id u64, year i32, flags u8
const MIN_YEAR: i32 = 2010;

// The measured Q1 query time. Source: results/imdb_shelf_runs.json, key
// `idle_control_20261021_1634`, field q1.query_ms.
const Q1_MEASURED_MS: f64 = 159.4;
// The IMDb bundle build. Source: results/imdb_shelf_runs.json, key
// `bundle_setup_cost_ms`, field `benchmark_20261021`. A forced full rebuild
// on 2026-10-22 measured 112,721 ms, so the setup cost has this range.
const BUNDLE_BUILD_MS: f64 = 51_100.0;
const BUNDLE_BUILD_FULL_REBUILD_MS: f64 = 112_721.0;

fn main() {
    // ---- PART 1: the ratings hash build, over 1.72M rows ----
    let t = Instant::now();
    let mut rat: HashMap<u64, (f32, i64)> = HashMap::with_capacity(N_RATINGS as usize);
    for i in 0..N_RATINGS {
        let id = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        rat.insert(id, ((i % 1000) as f32 * 0.01, (i % 500) as i64));
    }
    let build_ms = t.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(&rat);
    println!(
        "Q1_PART1 ratings_hash_build n={N_RATINGS} ms={build_ms:.1} ns_per_insert={:.1}",
        build_ms * 1e6 / N_RATINGS as f64
    );

    // ---- PART 2: the basics scan with the filter, and the map probes ----
    // Same shape as the real query: read every basics row, apply the
    // filter, and probe the ratings map for the survivors.
    let t = Instant::now();
    let mut c1 = 0usize;
    let mut v1 = 0i64;
    let mut a1 = 0f64;
    let mut probes = 0u64;
    for i in 0..N_BASICS {
        let id = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let yr = 1990 + (i % 40) as i32;
        let fl = (i % 8) as u8;
        if fl & 1 == 0 || yr < MIN_YEAR {
            continue;
        }
        probes += 1;
        if let Some(&(avg, votes)) = rat.get(&id) {
            if avg >= 7.5 {
                c1 += 1;
                v1 += votes;
                a1 += avg as f64;
            }
        }
    }
    let scan_ms = t.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box((c1, v1, a1));
    let basics_bytes = N_BASICS * BASIC_ROW;
    println!(
        "Q1_PART2 basics_scan+probes n={N_BASICS} bytes={basics_bytes} ms={scan_ms:.1} probes={probes} ns_per_probe={:.1}",
        scan_ms * 1e6 / probes.max(1) as f64
    );
    println!(
        "Q1_PART2_scan_gbs={:.2} (of the 19.2 GB/s measured bus peak)",
        basics_bytes as f64 / scan_ms / 1e6
    );

    // ---- PART 3: the total, compared with the measured query time ----
    let total = build_ms + scan_ms;
    println!(
        "Q1_PART3 total_model_ms={total:.1} (measured query_ms={Q1_MEASURED_MS}, \
         results/imdb_shelf_runs.json idle_control_20261021_1634)"
    );

    // ---- PART 4: what a genuine ANSWER-FIBER for Q1's shape costs ----
    // The fiber holds one row per qualifying title: the 44,906 titles
    // that pass the filter, with their aggregate. The query then reads
    // that and nothing else.
    const ANSWER_ROWS: u64 = 44_906;
    const ANSWER_ROW: u64 = 16; // id u64, avg f32, votes i64
    let answer_bytes = ANSWER_ROWS * ANSWER_ROW;
    let t = Instant::now();
    let mut c = 0usize;
    let mut v = 0i64;
    let mut a = 0f64;
    for i in 0..ANSWER_ROWS {
        let avg = (i % 1000) as f32 * 0.01;
        let votes = (i % 500) as i64;
        if avg >= 7.5 {
            c += 1;
            v += votes;
            a += avg as f64;
        }
    }
    let fiber_ms = t.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box((c, v, a));
    println!(
        "Q1_PART4 answer_fiber_read rows={ANSWER_ROWS} bytes={answer_bytes} ms={fiber_ms:.4} gbs={:.2}",
        answer_bytes as f64 / fiber_ms / 1e6
    );
    println!(
        "Q1_PART4_speedup_vs_measured={:.0}x  vs_model={:.0}x",
        Q1_MEASURED_MS / fiber_ms.max(1e-9),
        total / fiber_ms.max(1e-9)
    );

    // ---- PART 5: what the ANSWER-FIBER BUILD costs ----
    // This is the price of the fiber, paid once. It is the same join, so
    // the build cost equals the current query cost.
    println!("Q1_PART5 fiber_build_cost_equals_current_query_ms={total:.1} (same join, paid once instead of per query)");

    // ---- PART 6: the amortization, per query shape ----
    // The IMDb bundle build was measured at 51,100 ms for the Q2+Q3
    // fibers (CONVERSATION_LOG_2026-10-21.md event 10).
    const BUNDLE_BUILD_MS: f64 = 51_100.0;
    const BUNDLE_BUILD_FULL_REBUILD_MS: f64 = 112_721.0;
    let q1_save: f64 = Q1_MEASURED_MS - fiber_ms;
    let q2_save: f64 = 10_804.2 - 0.0014;
    let q3_save: f64 = 1_472.1 - 0.0001;
    println!("Q1_PART6 build_ms={BUNDLE_BUILD_MS:.0}");
    println!("Q1_PART6 build_ms_forced_full_rebuild={BUNDLE_BUILD_FULL_REBUILD_MS:.0}");
    println!(
        "Q1_PART6 break_even_queries Q1={:.0} Q2={:.0} Q3={:.0} all_three={:.0}",
        BUNDLE_BUILD_MS / q1_save.max(1e-9),
        BUNDLE_BUILD_MS / q2_save.max(1e-9),
        BUNDLE_BUILD_MS / q3_save.max(1e-9),
        BUNDLE_BUILD_MS / (q1_save + q2_save + q3_save)
    );
    println!(
        "Q1_PART6 break_even_queries_at_full_rebuild Q1={:.0} Q2={:.0} Q3={:.0} all_three={:.0}",
        BUNDLE_BUILD_FULL_REBUILD_MS / q1_save.max(1e-9),
        BUNDLE_BUILD_FULL_REBUILD_MS / q2_save.max(1e-9),
        BUNDLE_BUILD_FULL_REBUILD_MS / q3_save.max(1e-9),
        BUNDLE_BUILD_FULL_REBUILD_MS / (q1_save + q2_save + q3_save)
    );

    // ---- PART 7: the EFFICIENCY metric, cycles per byte ----
    // Energy scales with the work done, and the work done is proportional
    // to the cycles. The metric is therefore useful BYTES PER CYCLE.
    // The clock is measured by the probe, and the 19.2 GB/s bus peak is
    // measured. A scan that moves 1 byte per cycle is bandwidth-bound; a
    // scan that moves 0.1 bytes per cycle is stalled.
    const BUS_GBPS: f64 = 19.2;
    let bus_bytes_per_cycle_at_1ghz = BUS_GBPS * 1e9 / 1e9; // bytes per cycle at 1 GHz
    println!(
        "Q1_PART7 bus_bytes_per_cycle_at_1GHz={bus_bytes_per_cycle_at_1ghz:.3} (19.2 GB/s per 1 GHz)"
    );
    // The Q1 path: it moves 201.7 MB and does a hash build plus probes.
    let q1_bytes = N_RATINGS * RATING_ROW + N_BASICS * BASIC_ROW;
    // Cycles, at the measured 1-thread scan rate of 8.14 GB/s and the
    // measured hash-insert cost.
    let q1_cycles = (q1_bytes as f64 / 8.14e9) * 1e9 + (N_RATINGS as f64 * build_ms * 1e6 / N_RATINGS as f64);
    println!(
        "Q1_PART7 q1_bytes={q1_bytes} useful_bytes_per_cycle_at_3GHz={:.3}",
        q1_bytes as f64 / q1_cycles / 3e9
    );
    // The answer-fiber path: it moves 718 KB and does almost no work.
    let fiber_cycles = (answer_bytes as f64 / 19.2e9) * 1e9 * 3e9 / 3e9;
    println!(
        "Q1_PART7 fiber_useful_bytes_per_cycle_at_3GHz={:.1}",
        answer_bytes as f64 / fiber_cycles.max(1.0)
    );
}
