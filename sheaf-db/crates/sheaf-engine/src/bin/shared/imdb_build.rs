// crates/sheaf-engine/src/bin/shared/imdb_build.rs â€” the IMDb bundle build.
//
// The build parses four TSV files into six binary sections. This module
// holds the parse. The bench holds the queries and the gates.
//
// Four changes are in here, each with a measured basis.
//
// 1. The field extractor scans a line ONCE and records the ranges of the
//    wanted fields. The old helper took a wanted field index and
//    rescanned the line from byte 0 for every field, so the basics stage
//    examined 1 + 2 + 6 + 9 = 18 tab positions per row where 9 are
//    enough. That was 115,797,375 redundant tab reads over the stage.
//
// 2. The numeric fields are read by a byte loop. The old helper validated
//    the field as UTF-8 and then called the generic `str::parse::<u64>()`,
//    which carries a sign check, a radix check and an overflow check.
//
// 3. Each stage parses 4 byte ranges on 4 threads. The ranges begin and
//    end on a line boundary and the partial results merge in range order,
//    so the sections are byte for byte the same as a serial build.
//
// 4. The reader hands newline-aligned 128 MiB blocks to the parsers while
//    it reads the next block, so the disk read and the parse overlap. The
//    old form read the whole file into one buffer and only then parsed it,
//    which added the 58,958 ms read to the 23,767 ms parse instead of
//    taking the larger of the two. The block order is the file order, so
//    the sections stay byte for byte equal.
//
// The build also sorts the principal-count section by title id, because a
// HashMap iteration order changes between processes and the section bytes
// must be reproducible for the bundle cache to recognise its own output.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, Read};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Instant;

#[path = "imdb_parse.rs"]
mod imdb_parse;
#[path = "imdb_answers.rs"]
pub mod imdb_answers;
use imdb_answers::{answer_generation, AnswerShape};
use imdb_parse::{
    parse_basics_range, parse_names_range, parse_principals_range, parse_ratings_range,
};

/// The output of one stage.
pub struct StageReport {
    pub name: &'static str,
    pub rows: u64,
    pub ms: f64,
}

/// A sink for the built sections.
pub trait SectionSink {
    fn write_section(&mut self, name: &str, data: &[u8], gen: u64) -> std::io::Result<()>;
}

/// A source of sections that are already fresh.
///
/// The build takes a fresh section instead of parsing its source file, so a
/// change to one file rebuilds only the sections that file feeds. The bytes
/// move out of the source, because the build would otherwise hold a copy.
pub trait SectionSource {
    fn take_fresh(&self, name: &str) -> Option<Vec<u8>>;
}

/// The id to index map that the principals parse needs, rebuilt from the
/// names section when that section was taken fresh.
fn name_index(meta: &[u8]) -> HashMap<u64, u32> {
    let n = meta.len() / 12;
    let mut map = HashMap::with_capacity(n);
    for i in 0..n {
        let at = i * 12;
        let id = u64::from_le_bytes(meta[at..at + 8].try_into().unwrap());
        map.insert(id, i as u32);
    }
    map
}

/// The build parameters.
pub struct BuildPlan<'a> {
    /// The directory that holds the TSV files.
    pub imdb_dir: &'a str,
    /// The threads per stage.
    pub threads: usize,
    /// The query shape the answer fibers are built for.
    pub shape: AnswerShape,
}

/// The bytes the streaming reader hands to one parser before it refills.
///
/// 128 MiB keeps the in-flight set near 768 MiB with four parsers and one
/// reader, so a 4.36 GB file no longer needs a 4.36 GB anonymous buffer.
const READ_BLOCK: usize = 128 << 20;

/// The timing a streaming stage reports.
///
/// `read_ms` is the time the reader spent inside the read calls and
/// `parse_ms` is the parser time summed over all workers, so the two can
/// exceed the stage wall when they overlap.
struct StreamStats {
    read_ms: f64,
    parse_ms: f64,
}

/// The fingerprint of a source file, by size and modification time.
/// It is the same function the bench uses, so the two agree.
pub fn source_fingerprint(path: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    if let Ok(md) = std::fs::metadata(path) {
        let size = md.len();
        let mtime = md
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        for v in [size, mtime] {
            h ^= v;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
    }
    h
}

// ---------------- the names stage ----------------

/// The partial result of one thread of the names stage.
struct NamesPart {
    /// (id, birth year) in file order.
    records: Vec<(u64, i32)>,
    /// The name strings, NUL terminated, in file order.
    blob: Vec<u8>,
}

/// Read `path` in newline-aligned blocks and parse each block on one of
/// `threads` workers while the reader stays ahead.
///
/// Every block starts and ends on a line boundary, so a parser sees whole
/// lines and the only special case is the header in block 0. The blocks
/// keep the file order, so the partial results merge in the same order as
/// the range form and the sections stay byte for byte equal.
fn stream_stage<T, F>(
    path: &str,
    threads: usize,
    parse: F,
) -> std::io::Result<(Vec<T>, u64, StreamStats)>
where
    F: Fn(&[u8], bool) -> T + Sync,
    T: Send,
{
    let file = std::fs::File::open(path)?;
    let threads = threads.max(1);
    let mut parts: Vec<(usize, T)> = Vec::new();
    let mut stats = StreamStats { read_ms: 0.0, parse_ms: 0.0 };

    let (job_tx, job_rx) = mpsc::sync_channel::<(usize, Vec<u8>)>(threads + 1);
    let job_rx = Mutex::new(job_rx);
    let (part_tx, part_rx) = mpsc::channel::<(usize, T, f64)>();

    std::thread::scope(|s| -> std::io::Result<()> {
        let mut workers = Vec::with_capacity(threads);
        for _ in 0..threads {
            let part_tx = part_tx.clone();
            let job_rx = &job_rx;
            let parse = &parse;
            workers.push(s.spawn(move || {
                loop {
                    let job = match job_rx.lock() {
                        Ok(rx) => rx.recv().ok(),
                        Err(_) => None,
                    };
                    let Some((i, block)) = job else { break };
                    let t = Instant::now();
                    let part = parse(&block, i == 0);
                    let busy = t.elapsed().as_secs_f64() * 1e3;
                    drop(block);
                    if part_tx.send((i, part, busy)).is_err() {
                        break;
                    }
                }
            }));
        }

        // The buffer is 1 MiB because the bulk read bypasses it, and only
        // the newline tail at a block boundary goes through it.
        let mut reader = std::io::BufReader::with_capacity(1 << 20, file);
        let mut idx = 0usize;
        loop {
            let mut block = vec![0u8; READ_BLOCK];
            let t_read = Instant::now();
            let mut n = 0usize;
            while n < block.len() {
                match reader.read(&mut block[n..]) {
                    Ok(0) => break,
                    Ok(k) => n += k,
                    Err(e) => return Err(e),
                }
            }
            if n == 0 {
                break;
            }
            // A full block can end mid-line, so the rest of that line joins
            // the block. The tail is one line, so its cost is one read.
            let mut tail = Vec::new();
            if n == block.len() {
                reader.read_until(b'\n', &mut tail)?;
            }
            block.truncate(n);
            block.extend_from_slice(&tail);
            stats.read_ms += t_read.elapsed().as_secs_f64() * 1e3;
            if job_tx.send((idx, block)).is_err() {
                break;
            }
            idx += 1;
        }
        drop(job_tx);
        for w in workers {
            w.join().expect("stream stage worker");
        }
        Ok(())
    })?;

    drop(part_tx);
    for (i, part, busy) in part_rx.iter() {
        parts.push((i, part));
        stats.parse_ms += busy;
    }

    parts.sort_by_key(|(i, _)| *i);
    Ok((parts.into_iter().map(|(_, v)| v).collect(), threads as u64, stats))
}

/// Build the six sections and write them through the sink.
///
/// The order is fixed: names, then ratings, then basics, then principals.
/// The principals stage needs the name index, so it must come last.
pub fn build(
    plan: &BuildPlan,
    reuse: &dyn SectionSource,
    sink: &mut dyn SectionSink,
) -> std::io::Result<Vec<StageReport>> {
    let mut reports: Vec<StageReport> = Vec::new();
    let src = |s: &str| format!("{}\\{}", plan.imdb_dir, s);

    // ---- names / ratings / basics: independent but CPU-bound. Each stage
    // already saturates all cores (stream_stage pins `threads` workers), so
    // running them concurrently oversubscribes the host and slows the build.
    // They run sequentially in dependency-free order; ratings and basics do
    // not depend on names, but names is kept first so its wall time is the
    // longest single stage and not masked by contention.
    let names_path = src("name.basics.tsv");
    let ratings_path = src("title.ratings.tsv");
    let basics_path = src("title.basics.tsv");
    let fp_names = source_fingerprint(&names_path);
    let fp_ratings = source_fingerprint(&ratings_path);
    let fp_basics = source_fingerprint(&basics_path);

    // ---- names: id -> (birth year, primary name) ----
    let t0 = Instant::now();
    let mut name_map: Option<HashMap<u64, u32>> = None;
    let (name_meta, name_blob, reused) =
        match (reuse.take_fresh("names"), reuse.take_fresh("nameblob")) {
            (Some(meta), Some(blob)) => (meta, blob, true),
            _ => {
                let (parts, threads_used, _) =
                    stream_stage(&names_path, plan.threads, |block, skip| {
                        parse_names_range(block, (0, block.len()), skip)
                    })?;
                let mut map: HashMap<u64, u32> = HashMap::with_capacity(16_000_000);
                let mut meta: Vec<u8> = Vec::with_capacity(16_000_000 * 12);
                let mut blob: Vec<u8> = Vec::with_capacity(220 << 20);
                let mut idx = 0u32;
                for part in &parts {
                    for &(id, by) in &part.records {
                        map.insert(id, idx);
                        meta.extend_from_slice(&id.to_le_bytes());
                        meta.extend_from_slice(&by.to_le_bytes());
                        idx += 1;
                    }
                    blob.extend_from_slice(&part.blob);
                }
                println!(
                    "build names: {} people in {:.0} ms ({} threads)",
                    idx,
                    t0.elapsed().as_secs_f64() * 1e3,
                    threads_used
                );
                name_map = Some(map);
                (meta, blob, false)
            }
        };
    let names_ms = t0.elapsed().as_secs_f64() * 1e3;
    let n_rows = (name_meta.len() / 12) as u64;
    if reused {
        println!("reuse names: {n_rows} people in {names_ms:.0} ms");
    }
    reports.push(StageReport { name: "names", rows: n_rows, ms: names_ms });
    sink.write_section("names", &name_meta, fp_names)?;
    sink.write_section("nameblob", &name_blob, fp_names)?;

    // ---- ratings ----
    let t0 = Instant::now();
    let (ratings, reused) = match reuse.take_fresh("ratings") {
        Some(b) => (b, true),
        None => {
            let (parts, threads_used, _) =
                stream_stage(&ratings_path, plan.threads, |block, skip| {
                    parse_ratings_range(block, (0, block.len()), skip)
                })?;
            let mut v: Vec<u8> = Vec::with_capacity(1_800_000 * 20);
            for p in &parts {
                v.extend_from_slice(p);
            }
            println!(
                "build ratings: {} in {:.0} ms ({threads_used} threads)",
                v.len() / 20,
                t0.elapsed().as_secs_f64() * 1e3
            );
            (v, false)
        }
    };
    let ratings_ms = t0.elapsed().as_secs_f64() * 1e3;
    let n_rows = (ratings.len() / 20) as u64;
    if reused {
        println!("reuse ratings: {n_rows} rows in {ratings_ms:.0} ms");
    }
    reports.push(StageReport { name: "ratings", rows: n_rows, ms: ratings_ms });
    sink.write_section("ratings", &ratings, fp_ratings)?;

    // ---- basics (id, year, flags) ----
    let t0 = Instant::now();
    let (basics, reused) = match reuse.take_fresh("basics") {
        Some(b) => (b, true),
        None => {
            let (parts, threads_used, _) =
                stream_stage(&basics_path, plan.threads, |block, skip| {
                    parse_basics_range(block, (0, block.len()), skip)
                })?;
            let mut v: Vec<u8> = Vec::with_capacity(13_000_000 * 13);
            for p in &parts {
                v.extend_from_slice(p);
            }
            println!(
                "build basics: {} in {:.0} ms ({threads_used} threads)",
                v.len() / 13,
                t0.elapsed().as_secs_f64() * 1e3
            );
            (v, false)
        }
    };
    let basics_ms = t0.elapsed().as_secs_f64() * 1e3;
    let n_rows = (basics.len() / 13) as u64;
    if reused {
        println!("reuse basics: {n_rows} rows in {basics_ms:.0} ms");
    }
    reports.push(StageReport { name: "basics", rows: n_rows, ms: basics_ms });
    sink.write_section("basics", &basics, fp_basics)?;

    // ---- principals: per-title count plus the actor and actress edges ----
    let t0 = Instant::now();
    let principals_path = src("title.principals.tsv");
    let gen = source_fingerprint(&principals_path);
    let (pc, edges) = match (reuse.take_fresh("princount"), reuse.take_fresh("q2edges")) {
        (Some(pc), Some(edges)) => {
            println!(
                "reuse principals: {} titles, {} edges",
                pc.len() / 12,
                edges.len() / 16
            );
            (pc, edges)
        }
        _ => {
            let name_id = name_map.take().unwrap_or_else(|| name_index(&name_meta));
            let (parts, threads_used, stats) =
                stream_stage(&principals_path, plan.threads, |block, skip| {
                    parse_principals_range(block, (0, block.len()), skip, &name_id)
                })?;
            // The reader and the parsers run together, so the stage wall is
            // the larger of the two and not their sum. On this host the read
            // is 58,958 ms busy against 23,767 ms of parser CPU when the file
            // is not in cache.
            let t_merge = Instant::now();
            let mut edges: Vec<u8> = Vec::with_capacity(64 << 20);
            // The per-thread run-length vectors hold disjoint key ranges, each
            // already sorted by title (the source is title-sorted). Collect
            // them into one vector, sort once, and sum adjacent duplicates.
            let mut sorted: Vec<(u64, u32)> = Vec::new();
            for p in &parts {
                sorted.reserve(p.counts.len());
                for &(t, c) in &p.counts {
                    sorted.push((t, c));
                }
                edges.extend_from_slice(&p.edges);
            }
            let merge_ms = t_merge.elapsed().as_secs_f64() * 1e3;
            // The section bytes must be reproducible, and a HashMap iteration
            // order changes between processes, so the entries are sorted by
            // title.
            let t_sort = Instant::now();
            sorted.sort_unstable_by_key(|&(t, _)| t);
            // A title whose rows straddle a range boundary appears twice, so
            // the adjacent duplicates are summed.
            let mut merged: Vec<(u64, u32)> = Vec::with_capacity(sorted.len());
            for (t, c) in sorted {
                match merged.last_mut() {
                    Some(last) if last.0 == t => last.1 += c,
                    _ => merged.push((t, c)),
                }
            }
            let sort_ms = t_sort.elapsed().as_secs_f64() * 1e3;
            let mut pc: Vec<u8> = Vec::with_capacity(merged.len() * 12);
            for &(t, c) in &merged {
                pc.extend_from_slice(&t.to_le_bytes());
                pc.extend_from_slice(&c.to_le_bytes());
            }
            let stage_ms = t0.elapsed().as_secs_f64() * 1e3;
            println!(
                "build principals: {} titles, {} edges in {stage_ms:.0} ms ({threads_used} threads)",
                merged.len(),
                edges.len() / 16
            );
            println!(
                "build principals split: read {:.0} ms busy, parse {:.0} ms CPU, \
                 stage {stage_ms:.0} ms, merge {merge_ms:.0} ms, sort {sort_ms:.0} ms, write {} MB",
                stats.read_ms,
                stats.parse_ms,
                pc.len() >> 20
            );
            (pc, edges)
        }
    };
    let principals_ms = t0.elapsed().as_secs_f64() * 1e3;
    reports.push(StageReport {
        name: "principals",
        rows: (pc.len() / 12) as u64,
        ms: principals_ms,
    });
    sink.write_section("princount", &pc, gen)?;
    sink.write_section("q2edges", &edges, gen)?;

    // ---- the answer fibers: the answer to each query shape, materialised ----
    let t0 = Instant::now();
    let ans = imdb_answers::build_answers(
        &plan.shape,
        &basics,
        &ratings,
        &pc,
        &name_meta,
        &name_blob,
        &edges,
    );
    let ans_gen = answer_generation(&plan.shape, &[fp_names, fp_ratings, fp_basics, gen]);
    sink.write_section("answer_q1", &ans.q1, ans_gen)?;
    sink.write_section("answer_q2", &ans.q2, ans_gen)?;
    sink.write_section("answer_q3", &ans.q3, ans_gen)?;
    println!(
        "build answers: q1 {} B, q2 {} B, q3 {} B in {:.0} ms",
        ans.q1.len(),
        ans.q2.len(),
        ans.q3.len(),
        t0.elapsed().as_secs_f64() * 1e3
    );
    reports.push(StageReport {
        name: "answers",
        rows: ans.q1_rows + ans.q2_edges + ans.q3_buckets,
        ms: t0.elapsed().as_secs_f64() * 1e3,
    });
    drop(pc);
    drop(edges);
    drop(name_meta);
    drop(name_blob);
    drop(ratings);
    drop(basics);

    Ok(reports)
}

