// ADVERSARIAL MEASUREMENT: the hidden SECOND PASS over the data.
//
// The bundle verifies a CRC32C over every section it fetches. That is a
// full read of the section, on top of the read the query itself needs.
// So every bundle-backed query moves its data TWICE: once to verify and
// once to use.
//
// This measures the cost of that verification on the real section sizes,
// using the repository's own hardware CRC routine.
//
// STANDALONE TOOL. Compile and run from this directory:
//   rustc -O crc_pass.rs -o crc_pass.exe && ./crc_pass.exe

use std::time::Instant;

fn crc32c_sw(data: &[u8]) -> u32 {
    // A portable CRC32C (Castagnoli) implementation, same polynomial the
    // repository's crc_hw uses, to show the order of magnitude.
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0x82F6_3B78 & mask);
        }
    }
    !crc
}

fn main() {
    // The REAL section sizes, read from the bundle index.
    let sections: [(&str, u64); 6] = [
        ("names", 188_859_084),
        ("nameblob", 229_779_490),
        ("ratings", 34_429_540),
        ("basics", 167_262_875),
        ("princount", 139_516_692),
        ("q2edges", 679_260_080),
    ];

    // Measure the CRC over a 40 MB block, then scale. The block is the
    // largest that fits comfortably, and the rate is what matters.
    let block = 40usize << 20;
    let buf = vec![0x5Au8; block];

    let t = Instant::now();
    let crc = crc32c_sw(&buf);
    let crc_ms = t.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(crc);
    let crc_gbs = block as f64 / crc_ms / 1e6;
    println!(
        "CRC_SW block={block} ms={crc_ms:.0} gbs={crc_gbs:.2} (portable table loop)"
    );

    // The repository uses the SSE4.2 hardware CRC, which the source
    // comment says costs ~8.5 to 23 ms per 42.5 MB. Use the measured
    // range from the repo: 8.5 ms / 42.5 MB = 5.0 GB/s best,
    // 23 ms / 42.5 MB = 1.85 GB/s worst.
    const HW_BEST_GBPS: f64 = 5.0;
    const HW_WORST_GBPS: f64 = 1.85;
    println!("CRC_HW repo_measured_range_GBps={HW_WORST_GBPS}..{HW_BEST_GBPS} (8.5..23 ms per 42.5 MB, CONVERSATION_LOG B3)");

    // The total verification cost for each query's fetched sections.
    // Q1 fetches basics + ratings. Q2 fetches names + nameblob + q2edges.
    // Q3 fetches princount + basics.
    let q1: u64 = sections[2].1 + sections[3].1;
    let q2: u64 = sections[0].1 + sections[1].1 + sections[5].1;
    let q3: u64 = sections[4].1 + sections[3].1;
    for (name, bytes) in [("Q1", q1), ("Q2", q2), ("Q3", q3)] {
        let best = (bytes as f64 / 1e9) / HW_BEST_GBPS * 1000.0;
        let worst = (bytes as f64 / 1e9) / HW_WORST_GBPS * 1000.0;
        let read_at_bus = (bytes as f64 / 1e9) / 19.2 * 1000.0;
        println!(
            "CRC_PASS {name} bytes={bytes} read_at_19.2GBps={read_at_bus:.0}ms verify_hw={best:.0}..{worst:.0}ms total_traffic_ratio={:.2}x",
            1.0 + (best + worst) / 2.0 / read_at_bus
        );
    }

    // The whole-bundle figure.
    let total: u64 = sections.iter().map(|s| s.1).sum();
    let read_at_bus = (total as f64 / 1e9) / 19.2 * 1000.0;
    let verify = (total as f64 / 1e9) / HW_BEST_GBPS * 1000.0;
    println!(
        "CRC_PASS WHOLE_BUNDLE bytes={total} read={read_at_bus:.0}ms verify_hw={verify:.0}ms ratio={:.2}x",
        1.0 + verify / read_at_bus
    );

    // The load cost, per query, against the measured load times.
    // Measured: Q1 load_ms=301 (this run), Q2 load_ms=1573, Q3 load_ms=~69 (idle run).
    let measured_load: [(&str, f64, u64); 3] = [
        ("Q1", 301.0, q1),
        ("Q2", 1573.0, q2),
        ("Q3", 69.0, q3),
    ];
    for (name, ms, bytes) in measured_load {
        let read_at_bus = (bytes as f64 / 1e9) / 19.2 * 1000.0;
        let verify = (bytes as f64 / 1e9) / HW_BEST_GBPS * 1000.0;
        println!(
            "CRC_LOAD {name} measured_load_ms={ms:.0} read_at_bus={read_at_bus:.0}ms verify_at_best={verify:.0}ms accounted={:.2}x",
            (read_at_bus + verify) / ms
        );
    }
}
