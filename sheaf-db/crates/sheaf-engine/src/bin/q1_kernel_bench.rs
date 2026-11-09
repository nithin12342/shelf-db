// crates/sheaf-engine/src/bin/q1_kernel_bench.rs — which code change actually
// moves the IMDb Q1 number.
//
// The Q1 query is a filter plus aggregate over a join. The current code
// (crates/sheaf-engine/src/bin/imdb_fiber_bench.rs, the Q1 block) does:
//   1. build a HashMap over 1,721,477 rating rows
//   2. scan 12,866,375 basics rows, filter, probe the map, accumulate
// Both steps are single-threaded.
//
// The 4-accumulator fix measured 1.40x on the TPC-H scan kernel
// (crates/sheaf-kernel/src/simd/mt_scan.rs). That kernel is a tight
// streaming loop. The Q1 path is a random-probe loop, so the fix may not
// transfer. This bench measures the variants on the REAL bundle sections:
//   A  1 thread, 1 map, 1 accumulator        (the current code)
//   B  1 thread, 1 map, 4 accumulators       (the mt_scan fix, transplanted)
//   C  4 threads, 4 sharded maps             (the structural change)
//   D  the answer fiber                      (the floor, 44,906 rows)
//
// Every variant must produce the same count, votes and average.
//
// Usage:
//   cargo run -p sheaf-engine --bin q1_kernel_bench --release
use std::collections::HashMap;
use std::time::Instant;

#[path = "shared/imdb_fiber.rs"]
mod imdb_fiber;
use imdb_fiber::{basics_flags, basics_id, basics_year, ratings_avg, ratings_id, ratings_votes,
                  BundleReader};

type Res = (usize, i64, f64);

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bundle = bench_args::required_path(&args, "--bundle");
    let min_year: i32 = bench_args::opt(&args, "--min-year", 2000);
    let min_avg: f32 = bench_args::opt(&args, "--min-avg", 7.5);
    let threads: usize = bench_args::opt(&args, "--threads", 4);

    let mut br = BundleReader::open(&bundle).expect("open");
    let t = Instant::now();
    let basics = br.section("basics").expect("basics");
    let ratings = br.section("ratings").expect("ratings");
    println!(
        "Q1KB_load_ms={:.1} basics_bytes={} ratings_bytes={}",
        t.elapsed().as_secs_f64() * 1e3,
        basics.len(),
        ratings.len()
    );

    let nb = basics.len() / 13;
    let nr = ratings.len() / 20;

    // ---- variant A: the current code ----
    let t = Instant::now();
    let a = variant_one(&basics, &ratings, nb, nr, 1, min_year, min_avg);
    let a_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("Q1KB_A_one_thread_one_acc ms={a_ms:.1} count={} votes={} avg={:.6}", a.0, a.1, a.2 / a.0 as f64);

    // ---- variant B: 4 accumulators, still 1 thread ----
    let t = Instant::now();
    let b = variant_four_acc(&basics, &ratings, nb, nr, min_year, min_avg);
    let b_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("Q1KB_B_one_thread_four_acc ms={b_ms:.1} count={} votes={} avg={:.6}", b.0, b.1, b.2 / b.0 as f64);
    println!("Q1KB_B_gain_x={:.3} (the mt_scan 1.40x fix, transplanted onto the Q1 path)", a_ms / b_ms);

    // ---- variant C: 4 threads, 4 sharded maps ----
    let t = Instant::now();
    let c = variant_sharded(&basics, &ratings, nb, nr, threads, min_year, min_avg);
    let c_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("Q1KB_C_{threads}_threads_sharded_maps ms={c_ms:.1} count={} votes={} avg={:.6}", c.0, c.1, c.2 / c.0 as f64);
    println!("Q1KB_C_gain_x={:.3}", a_ms / c_ms);

    // ---- variant D: the answer fiber ----
    let t = Instant::now();
    let d = variant_answer_fiber(&basics, &ratings, nb, nr, min_year, min_avg);
    let d_total_ms = t.elapsed().as_secs_f64() * 1e3;
    println!("Q1KB_D_answer_fiber_total_ms={d_total_ms:.3} count={} votes={} avg={:.6}", d.0, d.1, d.2 / d.0 as f64);

    // ---- the build price of the answer fiber, on its own ----
    let build_ms = answer_fiber_build_ms(&basics, &ratings, nb, nr, min_year, min_avg);
    println!(
        "Q1KB_E_answer_fiber_build_ms={build_ms:.1} (the price paid once, replacing the query)"
    );

    // ---- the gate: every variant must agree ----
    let ok = a.0 == b.0 && a.0 == c.0 && a.0 == d.0
        && a.1 == b.1 && a.1 == c.1 && a.1 == d.1
        && ((a.2 - b.2).abs() / a.2) < 1e-9
        && ((a.2 - c.2).abs() / a.2) < 1e-9
        && ((a.2 - d.2).abs() / a.2) < 1e-9;
    println!(
        "gate_q1_variants_agree={} (count {}, votes {})",
        if ok { "PASS" } else { "FAIL" },
        a.0,
        a.1
    );
}

/// The current production shape: one map, one accumulator, one thread.
fn variant_one(basics: &[u8], ratings: &[u8], nb: usize, nr: usize, accs: usize, min_year: i32, min_avg: f32) -> Res {
    let mut rat: HashMap<u64, (f32, i64)> = HashMap::with_capacity(nr);
    for i in 0..nr {
        rat.insert(
            ratings_id(ratings, i),
            (ratings_avg(ratings, i), ratings_votes(ratings, i)),
        );
    }
    let mut c1 = 0usize;
    let mut v1 = 0i64;
    let mut a1 = f64::NAN;
    if accs == 1 {
        let mut s = 0f64;
        for i in 0..nb {
            let id = basics_id(basics, i);
            let yr = basics_year(basics, i);
            let fl = basics_flags(basics, i);
            if fl & 1 == 0 || yr < min_year {
                continue;
            }
            if let Some(&(avg, votes)) = rat.get(&id) {
                if avg >= min_avg {
                    c1 += 1;
                    v1 += votes;
                    s += avg as f64;
                }
            }
        }
        a1 = s;
    }
    (c1, v1, a1)
}

/// The mt_scan fix transplanted: four independent accumulator chains.
fn variant_four_acc(basics: &[u8], ratings: &[u8], nb: usize, nr: usize, min_year: i32, min_avg: f32) -> Res {
    let mut rat: HashMap<u64, (f32, i64)> = HashMap::with_capacity(nr);
    for i in 0..nr {
        rat.insert(
            ratings_id(ratings, i),
            (ratings_avg(ratings, i), ratings_votes(ratings, i)),
        );
    }
    let mut s = [0f64; 4];
    let mut c1 = 0usize;
    let mut v1 = 0i64;
    for i in 0..nb {
        let id = basics_id(basics, i);
        let yr = basics_year(basics, i);
        let fl = basics_flags(basics, i);
        if fl & 1 == 0 || yr < min_year {
            continue;
        }
        if let Some(&(avg, votes)) = rat.get(&id) {
            if avg >= min_avg {
                c1 += 1;
                v1 += votes;
                s[i & 3] += avg as f64;
            }
        }
    }
    (c1, v1, s.iter().sum())
}

/// The structural change: the rating map is sharded by id, and the basics
/// rows are routed to the shard that owns their id. Each thread owns one
/// shard, so no map is shared and no lock is taken.
fn variant_sharded(basics: &[u8], ratings: &[u8], nb: usize, nr: usize, threads: usize, min_year: i32, min_avg: f32) -> Res {
    // Partition the rating rows and the basics rows by shard, ONCE, in a
    // sequential pass. Each thread then owns one shard of both, so no map
    // is shared and no thread scans the other's rows.
    let mut rat_rows: Vec<Vec<u32>> = vec![Vec::new(); threads];
    for i in 0..nr {
        let id = ratings_id(ratings, i);
        rat_rows[shard_of(id, threads)].push(i as u32);
    }
    let mut bas_rows: Vec<Vec<u32>> = vec![Vec::new(); threads];
    for i in 0..nb {
        let id = basics_id(basics, i);
        bas_rows[shard_of(id, threads)].push(i as u32);
    }
    let mut out: Vec<Res> = Vec::new();
    std::thread::scope(|s| {
        let mut handles = Vec::new();
        for t in 0..threads {
            let rr = &rat_rows[t];
            let br = &bas_rows[t];
            handles.push(s.spawn(move || {
                let mut rat: HashMap<u64, (f32, i64)> = HashMap::with_capacity(rr.len() + 1024);
                for &ri in rr.iter() {
                    let i = ri as usize;
                    rat.insert(
                        ratings_id(ratings, i),
                        (ratings_avg(ratings, i), ratings_votes(ratings, i)),
                    );
                }
                let mut c1 = 0usize;
                let mut v1 = 0i64;
                let mut sums = [0f64; 4];
                for &ri in br.iter() {
                    let i = ri as usize;
                    let id = basics_id(basics, i);
                    let yr = basics_year(basics, i);
                    let fl = basics_flags(basics, i);
                    if fl & 1 == 0 || yr < min_year {
                        continue;
                    }
                    if let Some(&(avg, votes)) = rat.get(&id) {
                        if avg >= min_avg {
                            c1 += 1;
                            v1 += votes;
                            sums[(i >> 3) & 3] += avg as f64;
                        }
                    }
                }
                (c1, v1, sums.iter().sum::<f64>())
            }));
        }
        for h in handles {
            out.push(h.join().expect("thread"));
        }
    });
    let c1: usize = out.iter().map(|r| r.0).sum();
    let v1: i64 = out.iter().map(|r| r.1).sum();
    let a1: f64 = out.iter().map(|r| r.2).sum();
    (c1, v1, a1)
}

/// The floor: the answer is already materialised, so the query reads 44,906
/// rows and touches nothing else. The build is timed separately, because
/// the build is paid once and the query is paid per run.
fn variant_answer_fiber(basics: &[u8], ratings: &[u8], nb: usize, nr: usize, min_year: i32, min_avg: f32) -> Res {
    let mut rat: HashMap<u64, (f32, i64)> = HashMap::with_capacity(nr);
    for i in 0..nr {
        rat.insert(
            ratings_id(ratings, i),
            (ratings_avg(ratings, i), ratings_votes(ratings, i)),
        );
    }
    // The build step: find the rows that pass the filter.
    let tb = Instant::now();
    let mut keep: Vec<(u64, f32, i64)> = Vec::new();
    for i in 0..nb {
        let id = basics_id(basics, i);
        let yr = basics_year(basics, i);
        let fl = basics_flags(basics, i);
        if fl & 1 == 0 || yr < min_year {
            continue;
        }
        if let Some(&(avg, votes)) = rat.get(&id) {
            if avg >= min_avg {
                keep.push((id, avg, votes));
            }
        }
    }
    let build_ms = tb.elapsed().as_secs_f64() * 1e3;
    // The query step: read the answer and aggregate it.
    let t = Instant::now();
    let mut c1 = 0usize;
    let mut v1 = 0i64;
    let mut s = [0f64; 4];
    for (i, &(_, avg, votes)) in keep.iter().enumerate() {
        c1 += 1;
        v1 += votes;
        s[i & 3] += avg as f64;
    }
    let query_ms = t.elapsed().as_secs_f64() * 1e3;
    println!(
        "Q1KB_D_detail build_ms={build_ms:.1} query_ms={query_ms:.4} rows={}",
        keep.len()
    );
    (c1, v1, s.iter().sum())
}

/// The build step of the answer fiber, timed on its own. This is the price
/// paid once, and it replaces the query that runs every time.
pub fn answer_fiber_build_ms(basics: &[u8], ratings: &[u8], nb: usize, nr: usize, min_year: i32, min_avg: f32) -> f64 {
    let mut rat: HashMap<u64, (f32, i64)> = HashMap::with_capacity(nr);
    for i in 0..nr {
        rat.insert(
            ratings_id(ratings, i),
            (ratings_avg(ratings, i), ratings_votes(ratings, i)),
        );
    }
    let t = Instant::now();
    let mut n = 0usize;
    for i in 0..nb {
        let id = basics_id(basics, i);
        let yr = basics_year(basics, i);
        let fl = basics_flags(basics, i);
        if fl & 1 == 0 || yr < min_year {
            continue;
        }
        if let Some(&(avg, votes)) = rat.get(&id) {
            if avg >= min_avg {
                n += 1;
                std::hint::black_box((avg, votes));
            }
        }
    }
    std::hint::black_box(n);
    t.elapsed().as_secs_f64() * 1e3
}

/// The shard that owns a key. A simple mix, so the load spreads.
fn shard_of(id: u64, threads: usize) -> usize {
    let mut h = id;
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    (h as usize) % threads
}
