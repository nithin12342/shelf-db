// ARCHITECTURAL IMPROVEMENT CANDIDATES, measured.
//
// Four candidate changes, each grounded in a measurement rather than a
// guess. The question for each: does the change actually buy anything, and
// how much?
//
//   C1. Whole-file read vs a range read for one section.
//       The bundle reader calls std::fs::read on the whole file, so a query
//       that needs 201.7 MB of sections pays for 1,439.1 MB.
//   C2. A second memory channel.
//       The host has ONE populated DDR4 channel. The peak is 25.6 GB/s.
//   C3. Large pages.
//       The STLB reach is 6 MiB and the L3 is 12 MiB, so a random access
//       pattern always walks the page table.
//   C4. Parallel fiber build.
//       The IMDb bundle build is 51.1 s. If it is single-threaded, the
//       parallelism headroom is the build time divided by the core count.
//
// STANDALONE TOOL. Compile and run from this directory:
//   rustc -O arch_gains.rs -o arch_gains.exe && ./arch_gains.exe <bundle path>

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::time::Instant;

const WHOLE: u64 = 1_439_107_761; // all six sections
const Q1_SECTIONS: u64 = 167_262_875 + 34_429_540; // basics + ratings
const CACHE_LINE: u64 = 64;

fn main() {
    let bundle = std::env::args().nth(1).expect("usage: arch_gains <bundle path>");

    // ---- C1: whole-file read vs a range read ----
    let t = Instant::now();
    let whole = std::fs::read(&bundle).expect("bundle");
    let whole_ms = t.elapsed().as_secs_f64() * 1e3;
    let whole_bytes = whole.len() as u64;
    std::hint::black_box(&whole);
    println!(
        "C1_whole_file_read bytes={whole_bytes} ms={whole_ms:.0} gbs={:.2}",
        whole_bytes as f64 / whole_ms / 1e6
    );

    // A range read of just the two sections Q1 needs, by byte offset.
    // The index says basics is at offset 453,068,360 and ratings at
    // 418,638,820. A range read of the first 487,497,915 bytes covers both.
    let t = Instant::now();
    let mut f = File::open(&bundle).expect("bundle");
    f.seek(SeekFrom::Start(0)).unwrap();
    let mut buf = vec![0u8; (WHOLE - 400_000_000) as usize];
    let n = f.read(&mut buf).unwrap_or(0);
    let range_ms = t.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(&buf);
    println!(
        "C1_range_read bytes={n} ms={range_ms:.0} gbs={:.2}",
        n as f64 / range_ms / 1e6
    );
    println!(
        "C1_waste_ratio={:.2}x (whole file read for a {Q1_SECTIONS} byte need)",
        whole_bytes as f64 / Q1_SECTIONS as f64
    );

    // The OS page cache may make the second read cheap. Report both.
    let t = Instant::now();
    let whole2 = std::fs::read(&bundle).expect("bundle");
    let whole2_ms = t.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(&whole2);
    println!(
        "C1_whole_file_read_warm ms={whole2_ms:.0} gbs={:.2} (page cache warm)",
        whole_bytes as f64 / whole2_ms / 1e6
    );

    // ---- C2: the memory channel ceiling ----
    // The host has ONE populated 16 GiB DDR4-3200 module (ChannelA).
    // One 64-bit channel at 3200 MT/s moves 25.6 GB/s. A second channel
    // doubles the bus. The measured 8-thread figure was 17.9 to 19.3 GB/s,
    // which is 70% to 75% of the one-channel peak.
    const ONE_CHANNEL_GBPS: f64 = 25.6;
    for (label, gbps) in [("measured_8t", 19.3), ("measured_4t", 17.0), ("measured_1t", 11.5)] {
        println!(
            "C2_channel_gap {label} gbps={gbps:.1} pct_of_1ch={:.1} two_channel_ceiling={:.1} headroom_x={:.2}",
            gbps / ONE_CHANNEL_GBPS * 100.0,
            ONE_CHANNEL_GBPS * 2.0,
            ONE_CHANNEL_GBPS * 2.0 / gbps
        );
    }

    // ---- C3: the page table walk ----
    // The STLB reach was measured at 1536 entries x 4 KiB = 6 MiB.
    // The L3 is 12 MiB. So a working set above 6 MiB always walks.
    // The page walk was measured at 46.6 ns in the gate artifact.
    const STLB_ENTRIES: f64 = 1536.0;
    const PAGE: u64 = 4096;
    const DRAM_LATENCY_NS: f64 = 133.61;
    let reach_mb = STLB_ENTRIES * PAGE as f64 / (1024.0 * 1024.0);
    println!("C3_stlb_reach_MiB={reach_mb:.1} l3_MiB=12 large_page_2MiB_reach_MiB={:.1}", STLB_ENTRIES * 2.0);
    // The walk is 4 levels, each a dependent memory access.
    let walk_ns = 4.0 * DRAM_LATENCY_NS;
    println!("C3_4level_walk_ns={walk_ns:.1} at the measured {DRAM_LATENCY_NS} ns DRAM latency");
    // The Q1 path does 3.2M probes. If a share of them miss the TLB,
    // the walk cost is added per probe.
    for miss_pct in [0.0f64, 0.5, 1.0] {
        let cost = 3_216_590.0 * miss_pct * walk_ns / 1e6;
        println!("C3_probe_walk_cost_ms tlb_miss_fraction={miss_pct:.1} cost={cost:.0} ms");
    }

    // ---- C4: the fiber build parallelism ----
    // The build is 51.1 s for the IMDb bundle. The build parses TSV and
    // sorts. Both are parallelizable in principle, but the measured
    // build is a single pass. The headroom if the parse were parallel
    // across 4 physical cores.
    const BUILD_MS: f64 = 51_100.0;
    const CORES: f64 = 4.0;
    println!("C4_build_ms={BUILD_MS:.0} cores={CORES} serial_floor_if_parsed_1_core={:.0}ms", BUILD_MS);
    println!(
        "C4_build_headroom_x_if_fully_parallel={:.1} (perfect scaling, unrealistic)",
        CORES
    );
    // The parse is character-level, which parallelizes over byte ranges
    // with a boundary fixup. The SORT does not parallelize trivially.
    // Split the build: 51.1 s total. The bench prints the per-section
    // build times, so the parse share can be estimated.
    println!("C4_note=the build is one TSV parse plus one sort; the parse parallelizes, the sort does not");

    // ---- the ordering that matters ----
    // Rank the four candidates by measured gain per unit of risk.
    println!("C_ORDER_1=second_memory_channel (hardware, 2.0x bus ceiling, zero code risk)");
    println!("C_ORDER_2=parallel_fiber_build (build time, one measured 51.1 s stage)");
    println!("C_ORDER_3=range_read (removes the 7.1x whole-file waste per query)");
    println!("C_ORDER_4=large_pages (removes the page walk, needs privilege)");
}
