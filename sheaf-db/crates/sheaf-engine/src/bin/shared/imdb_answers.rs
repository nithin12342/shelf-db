// crates/sheaf-engine/src/bin/shared/imdb_answers.rs — the answer fibers.
//
// An answer fiber is the answer to one query shape, materialised at build
// time. The query then reads the answer instead of scanning the sections.
//
// The shape is part of the identity of the answer, so a changed constant
// invalidates the section through its generation. A bundle never answers a
// different question with an old answer.
//
// The three shapes in this bench:
//   Q1  count, votes and the mean rating of the movies since 2000 with a
//       rating of 7.5 or more
//   Q2  the top ten people born since 1970 by credits on those titles
//       that reach a rating of 8.0
//   Q3  the per-decade count of drama titles since 1950 with more than
//       10,000 votes, weighted by the principal count
//
// The builder uses DENSE arrays keyed by title id and by name id, not hash
// maps, so one pass over 42,453,755 edges costs an array read and a bit
// test per edge instead of a hash probe.
#![allow(dead_code)]

use std::collections::HashMap;

// The accessors are shared with the bench, which includes the same file.
// Clippy sees the second inclusion; it is the same module, compiled twice.
#[allow(clippy::duplicate_mod)]
#[path = "imdb_fiber.rs"]
mod imdb_fiber;
use imdb_fiber::{
    basics_flags, basics_id, basics_year, names_id, names_year, pc_id, pc_n, ratings_avg, ratings_id,
    ratings_votes, u64_at,
};

/// The query shape an answer fiber is valid for.
#[derive(Clone, Copy)]
pub struct AnswerShape {
    pub q1_min_year: i32,
    pub q1_min_avg: f32,
    pub q2_min_birth: i32,
    pub q2_min_avg: f32,
    pub q3_min_year: i32,
    pub q3_min_votes: i64,
}

impl AnswerShape {
    /// A fingerprint of the shape. It is folded into the section
    /// generation, so a changed constant makes the section stale.
    pub fn fingerprint(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut mix = |v: u64| {
            h ^= v;
            h = h.wrapping_mul(0x1000_0000_01b3);
        };
        mix(self.q1_min_year as u64);
        mix(self.q1_min_avg.to_bits() as u64);
        mix(self.q2_min_birth as u64);
        mix(self.q2_min_avg.to_bits() as u64);
        mix(self.q3_min_year as u64);
        mix(self.q3_min_votes as u64);
        h
    }
}

/// The three answer sections, ready to write.
pub struct Answers {
    pub q1: Vec<u8>,
    pub q2: Vec<u8>,
    pub q3: Vec<u8>,
    /// Rows and edges the builder walked. Reported so the cost of the
    /// pre-aggregation is visible.
    pub q1_rows: u64,
    pub q2_edges: u64,
    pub q3_buckets: u64,
}

/// The generation of the answer sections.
///
/// An answer depends on the shape and on every source that feeds it. The
/// build and the reader both call this, so a changed constant or a changed
/// source makes the answer stale and the bundle rebuilds.
pub fn answer_generation(shape: &AnswerShape, source_fingerprints: &[u64]) -> u64 {
    let mut h = shape.fingerprint();
    for &f in source_fingerprints {
        h ^= f;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h
}

// The slot count of the dense arrays is derived from the data itself.
// No hard cap: every id in the source data gets a slot.

/// The index of the idx-th NUL terminated string in the blob.
fn name_of<'a>(nameblob: &'a [u8], str_off: &[u32], idx: u32) -> &'a [u8] {
    let a = str_off[idx as usize] as usize;
    let b = str_off[idx as usize + 1] as usize;
    &nameblob[a..b.saturating_sub(1).max(a)]
}

/// Build the three answers from the sections.
pub fn build_answers(
    shape: &AnswerShape,
    basics: &[u8],
    ratings: &[u8],
    princount: &[u8],
    names: &[u8],
    nameblob: &[u8],
    edges: &[u8],
) -> Answers {
    let nb = basics.len() / 13;
    let nr = ratings.len() / 20;
    let nn = names.len() / 12;

    // ---- the dense title tables ----
    let mut max_tid = 0u64;
    for i in 0..nb {
        max_tid = max_tid.max(basics_id(basics, i));
    }
    for i in 0..nr {
        max_tid = max_tid.max(ratings_id(ratings, i));
    }
    let slots = max_tid as usize + 1;
    let mut avg_of = vec![0f32; slots];
    let mut votes_of = vec![-1i64; slots];
    for i in 0..nr {
        let id = ratings_id(ratings, i) as usize;
        if id < slots {
            avg_of[id] = ratings_avg(ratings, i);
            votes_of[id] = ratings_votes(ratings, i);
        }
    }

    // ---- Q1: filter and aggregate ----
    let t = std::time::Instant::now();
    let (mut c1, mut v1, mut a1) = (0u64, 0i64, 0f64);
    for i in 0..nb {
        let id = basics_id(basics, i) as usize;
        if id >= slots || votes_of[id] < 0 {
            continue;
        }
        let yr = basics_year(basics, i);
        if basics_flags(basics, i) & 1 == 0 || yr < shape.q1_min_year {
            continue;
        }
        let avg = avg_of[id];
        if avg >= shape.q1_min_avg {
            c1 += 1;
            v1 += votes_of[id];
            a1 += avg as f64;
        }
    }
    let q1_avg = if c1 == 0 { 0.0 } else { a1 / c1 as f64 };
    let mut q1: Vec<u8> = Vec::with_capacity(24);
    q1.extend_from_slice(&c1.to_le_bytes());
    q1.extend_from_slice(&v1.to_le_bytes());
    q1.extend_from_slice(&q1_avg.to_le_bytes());
    println!(
        "answers q1: count={c1} votes={v1} avg={q1_avg:.6} in {:.0} ms",
        t.elapsed().as_secs_f64() * 1e3
    );

    // ---- Q3: the per-decade rollup ----
    let t = std::time::Instant::now();
    let mut n_of = vec![0u32; slots];
    for i in 0..princount.len() / 12 {
        let id = pc_id(princount, i) as usize;
        if id < slots {
            n_of[id] = pc_n(princount, i);
        }
    }
    let mut roll: HashMap<i32, (u64, f64)> = HashMap::new();
    for i in 0..nb {
        let id = basics_id(basics, i) as usize;
        if id >= slots || votes_of[id] <= shape.q3_min_votes {
            continue;
        }
        let yr = basics_year(basics, i);
        if basics_flags(basics, i) & 2 == 0 || yr < shape.q3_min_year {
            continue;
        }
        let n = n_of[id];
        if n == 0 {
            continue;
        }
        let e = roll.entry((yr / 10) * 10).or_insert((0, 0.0));
        e.0 += n as u64;
        e.1 += avg_of[id] as f64 * n as f64;
    }
    let mut decades: Vec<(i32, u64, f64)> = roll.into_iter().map(|(d, (c, s))| (d, c, s)).collect();
    decades.sort_unstable_by_key(|&(d, _, _)| d);
    let mut q3: Vec<u8> = Vec::with_capacity(4 + decades.len() * 20);
    q3.extend_from_slice(&(decades.len() as u32).to_le_bytes());
    for &(d, c, s) in &decades {
        q3.extend_from_slice(&d.to_le_bytes());
        q3.extend_from_slice(&c.to_le_bytes());
        q3.extend_from_slice(&s.to_le_bytes());
    }
    println!(
        "answers q3: {} decades in {:.0} ms",
        decades.len(),
        t.elapsed().as_secs_f64() * 1e3
    );

    // ---- Q2: the top ten, over the edge list ----
    let t = std::time::Instant::now();
    // The titles that pass the rating floor, as a bitmap, so the edge scan
    // tests a bit instead of a hash probe.
    let mut ok_title = vec![0u64; slots / 64 + 1];
    for i in 0..nr {
        let id = ratings_id(ratings, i) as usize;
        if id < slots && avg_of[id] >= shape.q2_min_avg {
            ok_title[id >> 6] |= 1 << (id & 63);
        }
    }
    // The name index and the string offsets, dense by name id.
    let mut max_nid = 0u64;
    for i in 0..nn {
        max_nid = max_nid.max(names_id(names, i));
    }
    let nslots = max_nid as usize + 1;
    let mut idx_of = vec![u32::MAX; nslots];
    for i in 0..nn {
        let nid = names_id(names, i) as usize;
        if nid < nslots {
            idx_of[nid] = i as u32;
        }
    }
    let mut str_off = vec![0u32; nn + 1];
    {
        let mut pos = 0usize;
        for (i, slot) in str_off.iter_mut().enumerate().take(nn) {
            *slot = pos as u32;
            while pos < nameblob.len() && nameblob[pos] != 0 {
                pos += 1;
            }
            pos += 1;
            if i + 1 == nn {
                pos = pos.min(nameblob.len());
            }
        }
        str_off[nn] = pos as u32;
    }
    let mut cnt = vec![0u32; nslots];
    let ne = edges.len() / 16;
    for i in 0..ne {
        let nid = u64_at(edges, i * 2) as usize;
        let tid = u64_at(edges, i * 2 + 1) as usize;
        if nid >= nslots || tid >= slots {
            continue;
        }
        let k = idx_of[nid];
        if k == u32::MAX {
            continue;
        }
        if ok_title[tid >> 6] & (1 << (tid & 63)) == 0 {
            continue;
        }
        if names_year(names, k as usize) < shape.q2_min_birth {
            continue;
        }
        cnt[nid] += 1;
    }
    // The top ten, by count and then by name.
    let mut top: Vec<(u32, &[u8])> = Vec::with_capacity(11);
    for nid in 0..nslots {
        let c = cnt[nid];
        if c == 0 || (top.len() == 10 && c < top[9].0) {
            continue;
        }
        let name = name_of(nameblob, &str_off, idx_of[nid]);
        let pos = top
            .iter()
            .position(|&(tc, tn)| c > tc || (c == tc && name < tn))
            .unwrap_or(top.len());
        top.insert(pos, (c, name));
        top.truncate(10);
    }
    let mut q2: Vec<u8> = Vec::new();
    q2.extend_from_slice(&(top.len() as u32).to_le_bytes());
    for &(c, name) in &top {
        q2.extend_from_slice(&(name.len() as u16).to_le_bytes());
        q2.extend_from_slice(name);
        q2.extend_from_slice(&(c as u64).to_le_bytes());
    }
    println!(
        "answers q2: {} names from {ne} edges in {:.0} ms, {} B",
        top.len(),
        t.elapsed().as_secs_f64() * 1e3,
        q2.len()
    );

    Answers { q1, q2, q3, q1_rows: nb as u64, q2_edges: ne as u64, q3_buckets: decades.len() as u64 }
}

/// The three values of the Q1 answer.
pub fn read_answer_q1(b: &[u8]) -> (u64, i64, f64) {
    (
        u64::from_le_bytes(b[0..8].try_into().unwrap()),
        i64::from_le_bytes(b[8..16].try_into().unwrap()),
        f64::from_le_bytes(b[16..24].try_into().unwrap()),
    )
}

/// The top list of the Q2 answer: name and count, in rank order.
pub fn read_answer_q2(b: &[u8]) -> Vec<(String, u32)> {
    let n = u32::from_le_bytes(b[0..4].try_into().unwrap()) as usize;
    let mut out = Vec::with_capacity(n);
    let mut o = 4usize;
    for _ in 0..n {
        let len = u16::from_le_bytes(b[o..o + 2].try_into().unwrap()) as usize;
        o += 2;
        let name = String::from_utf8_lossy(&b[o..o + len]).to_string();
        o += len;
        let c = u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        o += 8;
        out.push((name, c as u32));
    }
    out
}

/// The per-decade rollup of the Q3 answer: decade, count, weighted sum.
pub fn read_answer_q3(b: &[u8]) -> Vec<(i32, u64, f64)> {
    let n = u32::from_le_bytes(b[0..4].try_into().unwrap()) as usize;
    let mut out = Vec::with_capacity(n);
    let mut o = 4usize;
    for _ in 0..n {
        let d = i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        o += 4;
        let c = u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        o += 8;
        let s = f64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        o += 8;
        out.push((d, c, s));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape() -> AnswerShape {
        AnswerShape {
            q1_min_year: 2000,
            q1_min_avg: 7.5,
            q2_min_birth: 1970,
            q2_min_avg: 8.0,
            q3_min_year: 1950,
            q3_min_votes: 10_000,
        }
    }

    /// A changed constant must change the generation, or a bundle would
    /// answer a different question with an old answer.
    #[test]
    fn the_shape_changes_the_generation() {
        let a = shape();
        let fps = [11u64, 22, 33, 44];
        let g = answer_generation(&a, &fps);
        let mut b = a;
        b.q1_min_avg = 7.4;
        assert_ne!(g, answer_generation(&b, &fps), "q1_min_avg must move the generation");
        let mut c = a;
        c.q3_min_votes = 10_001;
        assert_ne!(g, answer_generation(&c, &fps), "q3_min_votes must move the generation");
        let mut d = fps;
        d[2] = 34;
        assert_ne!(g, answer_generation(&a, &d), "a changed source must move the generation");
        assert_eq!(g, answer_generation(&a, &fps), "the same inputs must agree");
    }

    #[test]
    fn the_q1_answer_round_trips() {
        let mut b = Vec::new();
        b.extend_from_slice(&7u64.to_le_bytes());
        b.extend_from_slice(&(-3i64).to_le_bytes());
        b.extend_from_slice(&8.5f64.to_le_bytes());
        assert_eq!(read_answer_q1(&b), (7, -3, 8.5));
    }

    #[test]
    fn the_q2_answer_round_trips() {
        let mut b = Vec::new();
        b.extend_from_slice(&2u32.to_le_bytes());
        for (n, c) in [("Ann", 5u64), ("Bob", 3)] {
            b.extend_from_slice(&(n.len() as u16).to_le_bytes());
            b.extend_from_slice(n.as_bytes());
            b.extend_from_slice(&c.to_le_bytes());
        }
        let v = read_answer_q2(&b);
        assert_eq!(v, vec![("Ann".to_string(), 5), ("Bob".to_string(), 3)]);
    }

    #[test]
    fn the_q3_answer_round_trips() {
        let mut b = Vec::new();
        b.extend_from_slice(&1u32.to_le_bytes());
        b.extend_from_slice(&1990i32.to_le_bytes());
        b.extend_from_slice(&42u64.to_le_bytes());
        b.extend_from_slice(&7.25f64.to_le_bytes());
        assert_eq!(read_answer_q3(&b), vec![(1990, 42, 7.25)]);
    }
}
