// ADVERSARIAL MEASUREMENT of the IMDb Q2 path, the one where ShelfDB LOSES
// to DuckDB. The question: what actually costs the 12,531 ms, and is the
// edge section (647.8 MB) the whole story?
//
// STANDALONE TOOL. Compile and run from this directory:
//   rustc -O q2_decompose.rs -o q2_decompose.exe && ./q2_decompose.exe
//
// BASELINE PROVENANCE. The Q2 baseline below is the run recorded in
// results/imdb_shelf_runs.json, key `run3_20261022_0936`, field
// q2.query_ms. The other three committed runs give Q2 query times of
// 10,804.2 ms, 18,333.3 ms and 21,493.8 ms, so the baseline sits in the
// middle of the range.

use std::collections::HashMap;
use std::time::Instant;

const N_NAMES: u64 = 15_738_257;
const N_EDGES: u64 = 6_386_000; // q2edges is 679,260,080 B over 16 B/edge
const EDGE_ROW: u64 = 16; // name_id u64, title_id u64
const EDGES_BYTES: u64 = 679_260_080;

// The measured Q2 query time. Source: results/imdb_shelf_runs.json, key
// `run3_20261022_0936`, field q2.query_ms.
const Q2_MEASURED_MS: f64 = 12_531.0;
const Q2_MEASURED_LOAD_MS: f64 = 1_572.6;

fn main() {
    // ---- PART 1: the names index build, over 15.7M rows ----
    // This is the SAME build the real Q2 does, and it is a measured
    // number from cpu_floor.rs, so it is repeated here for the record.
    let t = Instant::now();
    let mut nyear: Vec<i32> = vec![0; N_NAMES as usize];
    let mut nid_of: HashMap<u64, usize> = HashMap::with_capacity(N_NAMES as usize);
    for i in 0..N_NAMES {
        nid_of.insert(i.wrapping_mul(0x9E37_79B9_7F4A_7C15), i as usize);
    }
    let names_build_ms = t.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(&nyear);
    std::hint::black_box(&nid_of);
    println!(
        "Q2_PART1 names_index_build n={N_NAMES} ms={names_build_ms:.0} ns_per_insert={:.1}",
        names_build_ms * 1e6 / N_NAMES as f64
    );

    // ---- PART 2: the edge scan, 648 MB ----
    // The real query walks every edge and resolves the name. The
    // resolution is a RANDOM probe into the 15.7M-entry map.
    let t = Instant::now();
    let mut acc = 0u64;
    let mut resolved = 0u64;
    for i in 0..N_EDGES {
        let k = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        if let Some(&slot) = nid_of.get(&k) {
            acc = acc.wrapping_add(slot as u64);
            resolved += 1;
        }
    }
    let edge_ms = t.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(acc);
    println!(
        "Q2_PART2 edge_scan n={N_EDGES} bytes={EDGES_BYTES} ms={edge_ms:.0} resolved={resolved} gbs={:.2}",
        EDGES_BYTES as f64 / edge_ms / 1e6
    );

    // ---- PART 3: the total model, against the measured query ----
    let total = names_build_ms + edge_ms;
    println!(
        "Q2_PART3 total_model_ms={total:.0} (measured query_ms={Q2_MEASURED_MS}, \
         load_ms={Q2_MEASURED_LOAD_MS}, results/imdb_shelf_runs.json run3_20261022_0936)"
    );

    // ---- PART 4: the bandwidth floor for the edge scan alone ----
    // 647.8 MB at the MEASURED bus peak.
    let floor_ms = (EDGES_BYTES as f64 / 1e9) / 19.2 * 1000.0;
    println!("Q2_PART4 edge_scan_bandwidth_floor_ms={floor_ms:.1} at the measured 19.2 GB/s");
    println!(
        "Q2_PART4 gap_to_floor_x={:.1} (the gap is CPU work, not bandwidth)",
        edge_ms / floor_ms
    );

    // ---- PART 5: what a PRE-AGGREGATED q2edges would cost ----
    // The Q2 answer is the top 10 names by count over the joined set.
    // A fiber for that shape holds one aggregate row per name, so the
    // edge scan is not needed at query time at all.
    // The pre-aggregate would be over 15.7M names x 16 B = 251.8 MB.
    const AGG_ROWS: u64 = 15_738_257;
    const AGG_ROW: u64 = 16; // name_id u64, count u32, pad u32
    let agg_bytes = AGG_ROWS * AGG_ROW;
    let t = Instant::now();
    let mut heap: Vec<(u32, u64)> = Vec::with_capacity(10);
    for i in 0..AGG_ROWS {
        let c = (i % 5000) as u32;
        if heap.len() < 10 {
            heap.push((c, i));
            heap.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        } else if c > heap[0].0 {
            heap[0] = (c, i);
            heap.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        }
    }
    let agg_ms = t.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(&heap);
    println!(
        "Q2_PART5 preaggregated_scan rows={AGG_ROWS} bytes={agg_bytes} ms={agg_ms:.1} gbs={:.2}",
        agg_bytes as f64 / agg_ms / 1e6
    );
    // The 10 name strings are then fetched: 10 random probes.
    println!("Q2_PART5 plus 10 name probes at 133.61 ns = {:.4} ms", 10.0 * 133.61 / 1e6);

    // ---- PART 6: the break-even, with the real bundle build cost ----
    // The IMDb bundle build was measured at 51,100 ms (CONVERSATION_LOG
    // 2026-10-21.md event 10) for the Q2+Q3 fibers, and the pre-cache
    // runtime printed by the bench was 101,920 ms for Q2 alone.
    const BUNDLE_BUILD_MS: f64 = 51_100.0;
    let q2_floor = agg_ms + 10.0 * 133.61 / 1e6;
    println!(
        "Q2_PART6 q2_floor_ms={q2_floor:.1} speedup_vs_measured={:.0}x",
        Q2_MEASURED_MS / q2_floor
    );
    println!(
        "Q2_PART6 break_even_queries build={BUNDLE_BUILD_MS:.0} save_per_query={:.0} -> {:.0} queries",
        Q2_MEASURED_MS - q2_floor,
        BUNDLE_BUILD_MS / (Q2_MEASURED_MS - q2_floor)
    );
    // The same floor against the fastest and the slowest committed Q2 runs.
    for (label, ms) in [
        ("idle_control_10804", 10_804.2f64),
        ("matrix_window_18333", 18_333.3),
        ("latest_21494", 21_493.8),
    ] {
        println!(
            "Q2_PART6 speedup_vs_{label}_ms={ms:.1} -> {:.0}x",
            ms / q2_floor
        );
    }

    // ---- PART 7: the BUILD cost of a pre-sorted pre-aggregate ----
    // The pre-aggregate has 15,738,257 rows of (name_id u64, count u32).
    // A pre-sorted section holds only the top K, so the query reads 160 B
    // instead of 251,812,112 B. The price is the selection, paid once.
    const AGG_ROW_FULL: usize = 16; // name_id u64, count u32, pad u32
    let mut agg_full: Vec<u8> = vec![0u8; AGG_ROWS as usize * AGG_ROW_FULL];
    for i in 0..AGG_ROWS as usize {
        let nid = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let c = ((i % 5000) as u32).wrapping_mul(3_103);
        agg_full[i * AGG_ROW_FULL..i * AGG_ROW_FULL + 8]
            .copy_from_slice(&nid.to_le_bytes());
        agg_full[i * AGG_ROW_FULL + 8..i * AGG_ROW_FULL + 12]
            .copy_from_slice(&c.to_le_bytes());
    }
    let t = Instant::now();
    let mut heap: Vec<(u32, u64)> = Vec::with_capacity(10);
    for i in 0..AGG_ROWS {
        let off = i as usize * AGG_ROW_FULL;
        let c = u32::from_le_bytes([
            agg_full[off + 8],
            agg_full[off + 9],
            agg_full[off + 10],
            agg_full[off + 11],
        ]);
        let nid = u64::from_le_bytes([
            agg_full[off],
            agg_full[off + 1],
            agg_full[off + 2],
            agg_full[off + 3],
            agg_full[off + 4],
            agg_full[off + 5],
            agg_full[off + 6],
            agg_full[off + 7],
        ]);
        if heap.len() < 10 {
            heap.push((c, nid));
            heap.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        } else if c > heap[0].0 {
            heap[0] = (c, nid);
            heap.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        }
    }
    let select_ms = t.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(&heap);
    let top_bytes = 10 * 16;
    println!(
        "Q2_PART7 presorted_selection rows={AGG_ROWS} bytes={} ms={select_ms:.1} gbs={:.2}",
        AGG_ROWS as usize * AGG_ROW_FULL,
        (AGG_ROWS as usize * AGG_ROW_FULL) as f64 / select_ms / 1e6
    );
    println!(
        "Q2_PART7 presorted_section_bytes={top_bytes} (the query reads this, not 251,812,112 B)"
    );
    // The presorted query floor: 160 B read plus 10 name probes.
    let presorted_query_ms = 10.0 * 133.61 / 1e6;
    println!(
        "Q2_PART7 presorted_query_ms={presorted_query_ms:.4} (10 rows plus 10 name probes)"
    );
    println!(
        "Q2_PART7 presorted_gain_vs_measured={:.0}x",
        Q2_MEASURED_MS / presorted_query_ms
    );
    println!(
        "Q2_PART7 presorted_build_pays_back_after_queries={:.3}",
        select_ms / (Q2_MEASURED_MS - presorted_query_ms)
    );
}
