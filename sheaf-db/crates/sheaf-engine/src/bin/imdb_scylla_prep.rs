// crates/sheaf-engine/src/bin/imdb_scylla_prep.rs — Rust-side write-time join
// for the ScyllaDB (CQL) IMDb case. The join is the expensive part (12.9M
// basics + 1.7M ratings); doing it in Rust keeps the Python side to a thin
// CQL transport (insert + query), per the "Rust first" directive.
//
// Output: the path given by --out
//   [MAGIC u64][count u64] then count x (title_id u64, year i32, avg f32, votes i64)
// Rows: movies with year >= MIN_YEAR that have a rating. The query's
// avg >= MIN_AVG filter is NOT applied here — it stays a query-time
// clustering-range scan on the Scylla side.
use std::collections::HashMap;
use std::time::Instant;

const MIN_YEAR: i32 = 2000;
const MAGIC: u64 = 0x5343_594C_524F_5753; // "SCYLROWS"

fn id_of(b: &[u8]) -> Option<u64> {
    // "tt1234567" -> 1234567
    if b.len() < 3 {
        return None;
    }
    std::str::from_utf8(&b[2..]).ok()?.parse().ok()
}

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let imdb = bench_args::required_path(&args, "--imdb");
    let out_path = bench_args::required_path(&args, "--out");

    // --- ratings into a hash (1.7M) ---
    let t0 = Instant::now();
    let content = std::fs::read(format!("{imdb}\\title.ratings.tsv")).expect("ratings tsv");
    let mut ratings: HashMap<u64, (f32, i64)> = HashMap::with_capacity(1_800_000);
    let mut start = 0usize;
    let mut first = true;
    for i in 0..content.len() {
        if content[i] == b'\n' {
            let line = &content[start..i];
            start = i + 1;
            if line.is_empty() {
                continue;
            }
            if first {
                first = false;
                continue;
            }
            let mut fs = 0usize;
            let mut fstart = 0usize;
            let (mut id, mut avg, mut votes) = (0u64, 0f32, 0i64);
            for j in 0..line.len() {
                if line[j] == b'\t' {
                    match fs {
                        0 => id = id_of(&line[fstart..j]).unwrap_or(0),
                        1 => avg = std::str::from_utf8(&line[fstart..j]).unwrap_or("0").parse().unwrap_or(0.0),
                        2 => votes = std::str::from_utf8(&line[fstart..j]).unwrap_or("0").parse().unwrap_or(0),
                        _ => {}
                    }
                    fs += 1;
                    fstart = j + 1;
                }
            }
            // The LAST field of a TSV line has no trailing tab: it is the
            // remainder after the last separator. Parsing only on '\t' loses
            // numVotes (all votes came out 0).
            if fs == 2 {
                votes = std::str::from_utf8(&line[fstart..]).unwrap_or("0").parse().unwrap_or(0);
            }
            if id != 0 {
                ratings.insert(id, (avg, votes));
            }
        }
    }
    let ratings_load_ms = t0.elapsed().as_secs_f64() * 1e3;

    // --- stream basics, join, emit ---
    let t0 = Instant::now();
    let content = std::fs::read(format!("{imdb}\\title.basics.tsv")).expect("basics tsv");
    let mut out: Vec<u8> = Vec::with_capacity(64 << 20);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&0u64.to_le_bytes());
    let mut count = 0u64;
    let mut start = 0usize;
    let mut first = true;
    for i in 0..content.len() {
        if content[i] != b'\n' {
            continue;
        }
        let line = &content[start..i];
        start = i + 1;
        if line.is_empty() {
            continue;
        }
        if first {
            first = false;
            continue;
        }
        // fields: tconst(0) titleType(1) ... startYear(5) ... genres(8)
        let mut fs = 0usize;
        let mut fstart = 0usize;
        let (mut tid, mut ty, mut yr) = (0u64, false, 0i32);
        for j in 0..line.len() {
            if line[j] == b'\t' {
                match fs {
                    0 => tid = id_of(&line[fstart..j]).unwrap_or(0),
                    1 => ty = &line[fstart..j] == b"movie",
                    5 => {
                        let s = &line[fstart..j];
                        yr = if s.len() == 4 {
                            std::str::from_utf8(s).unwrap_or("0").parse().unwrap_or(0)
                        } else {
                            0
                        }
                    }
                    _ => {}
                }
                fs += 1;
                fstart = j + 1;
            }
        }
        if !ty || yr < MIN_YEAR || tid == 0 {
            continue;
        }
        if let Some(&(avg, votes)) = ratings.get(&tid) {
            out.extend_from_slice(&tid.to_le_bytes());
            out.extend_from_slice(&yr.to_le_bytes());
            out.extend_from_slice(&avg.to_le_bytes());
            out.extend_from_slice(&votes.to_le_bytes());
            count += 1;
        }
    }
    out[8..16].copy_from_slice(&count.to_le_bytes());
    std::fs::write(&out_path, &out).expect("write rows");
    let join_ms = t0.elapsed().as_secs_f64() * 1e3;
    println!(
        "imdb_scylla_prep: rows={count} bytes={} ratings_load_ms={ratings_load_ms:.0} join_emit_ms={join_ms:.0} out={out_path}",
        out.len()
    );
}
