// Ground the CPU cost of the IMDb Q2/Q3 queries: the hash-map build and
// the random name probes, which dominate over the bandwidth floor.
// STANDALONE TOOL. Compile and run from this directory:
//   rustc -O cpu_floor.rs -o cpu_floor.exe && ./cpu_floor.exe
// It is NOT part of the cargo workspace, because it measures the cost of
// work the engine does, not the engine itself.
use std::collections::HashMap;
use std::time::Instant;

fn main() {
    // Q2/Q3 build a u64 -> u32 map over the NAMES section.
    let n_names: u64 = 15_738_257;
    let n_ratings: u64 = 1_721_477;

    // 1. HashMap build over 15.7M u64 keys.
    let t0 = Instant::now();
    let mut m: HashMap<u64, u32> = HashMap::with_capacity(n_names as usize);
    for i in 0..n_names {
        m.insert(i.wrapping_mul(0x9E37_79B9_7F4A_7C15), (i % 200) as u32);
    }
    let build_ms = t0.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(&m);
    println!(
        "HASHBUILD n={n_names} ms={build_ms:.0} ns_per_insert={:.1}",
        build_ms * 1e6 / n_names as f64
    );

    // 2. Random probes into the same table: the name lookup per rating.
    let t1 = Instant::now();
    let mut acc = 0u64;
    for i in 0..n_ratings {
        let k = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        if let Some(v) = m.get(&k) {
            acc += *v as u64;
        }
    }
    let probe_ms = t1.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(acc);
    println!(
        "HASHPROBE n={n_ratings} ms={probe_ms:.0} ns_per_probe={:.1}",
        probe_ms * 1e6 / n_ratings as f64
    );

    // 3. Sequential scan of the same table, for the bandwidth reference.
    let t2 = Instant::now();
    let mut s = 0u64;
    for i in 0..n_names {
        s = s.wrapping_add(i);
    }
    let scan_ms = t2.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(s);
    let bytes = n_names * 12;
    println!(
        "SEQSCAN n={n_names} bytes={bytes} ms={scan_ms:.0} gbs={:.2}",
        bytes as f64 / scan_ms / 1e6
    );

    // 4. The floor for a query that only needs the ANSWER, pre-aggregated.
    //    Q1's answer is 44,906 rows of 16 B.
    let answer_bytes = 44_906u64 * 16;
    let t3 = Instant::now();
    let mut a = 0u64;
    for i in 0..44_906u64 {
        a = a.wrapping_add(i * 16);
    }
    let ans_ms = t3.elapsed().as_secs_f64() * 1e3;
    std::hint::black_box(a);
    println!("ANSWERONLY rows=44906 bytes={answer_bytes} ms={ans_ms:.4}");
}
