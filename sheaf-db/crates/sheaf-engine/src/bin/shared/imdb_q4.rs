// crates/sheaf-engine/src/bin/shared/imdb_q4.rs — 5-table JOB complex query Q4.
//
// Evaluates:
//   title_basics (b) + title_ratings (r) + title_principals (p) + name_basics (n)
// with predicates:
//   b.titletype = 'movie' AND b.startyear >= 1980
//   r.averagerating >= 7.8 AND r.numvotes >= 50000
//   p.category IN ('actor', 'actress')
//   n.birthyear >= 1950
//   HAVING COUNT(*) >= 5
//   ORDER BY count DESC, avg_rating DESC, name ASC LIMIT 10
//
// The default shape is the JOB Q4 shape. Every threshold is a CLI flag:
//   --q4-min-year, --q4-min-rating, --q4-min-votes, --q4-min-birth-year,
//   --q4-min-count, --q4-limit
#![allow(dead_code)]

use std::time::Instant;

use super::bench_args;
use super::imdb_fiber::{
    basics_flags, basics_id, basics_year, names_id, names_year, ratings_avg, ratings_id,
    ratings_votes, u64_at, BundleReader,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Q4Row {
    pub name: String,
    pub count: u32,
    pub avg: f64,
    pub votes: i64,
}

/// Default Q4 shape values. These are CLI-overridable.
const DEFAULT_Q4_MIN_YEAR: i32 = 1980;
const DEFAULT_Q4_MIN_RATING: f32 = 7.8;
const DEFAULT_Q4_MIN_VOTES: i64 = 50_000;
const DEFAULT_Q4_MIN_BIRTH_YEAR: i32 = 1950;
const DEFAULT_Q4_MIN_COUNT: u32 = 5;
const DEFAULT_Q4_LIMIT: usize = 10;

/// The Q4 predicate shape, parsed from the command line.
pub struct Q4Params {
    min_year: i32,
    min_rating: f32,
    min_votes: i64,
    min_birth_year: i32,
    min_count: u32,
    limit: usize,
}

impl Q4Params {
    /// Read the Q4 flags, with the JOB Q4 shape as the default.
    pub fn from_args(args: &[String]) -> Self {
        Self {
            min_year: bench_args::opt(args, "--q4-min-year", DEFAULT_Q4_MIN_YEAR),
            min_rating: bench_args::opt(args, "--q4-min-rating", DEFAULT_Q4_MIN_RATING),
            min_votes: bench_args::opt(args, "--q4-min-votes", DEFAULT_Q4_MIN_VOTES),
            min_birth_year: bench_args::opt(
                args,
                "--q4-min-birth-year",
                DEFAULT_Q4_MIN_BIRTH_YEAR,
            ),
            min_count: bench_args::opt(args, "--q4-min-count", DEFAULT_Q4_MIN_COUNT),
            limit: bench_args::opt(args, "--q4-limit", DEFAULT_Q4_LIMIT),
        }
    }
}

pub fn run_q4(
    br: &mut BundleReader,
    p: &Q4Params,
) -> Result<(Vec<Q4Row>, f64, f64), String> {
    let t_load = Instant::now();
    let basics = br.section("basics")?;
    let ratings = br.section("ratings")?;
    let names = br.section("names")?;
    let nameblob = br.section("nameblob")?;
    let edges = br.section("q2edges")?;
    let load_ms = t_load.elapsed().as_secs_f64() * 1e3;

    let t_exec = Instant::now();
    let nb = basics.len() / 13;
    let nr = ratings.len() / 20;
    let nn = names.len() / 12;
    let ne = edges.len() / 16;

    let mut max_tid = 0u64;
    for i in 0..nb {
        max_tid = max_tid.max(basics_id(&basics, i));
    }
    for i in 0..nr {
        max_tid = max_tid.max(ratings_id(&ratings, i));
    }
    let slots = max_tid as usize + 1;
    let mut ok_title = vec![0u64; slots / 64 + 1];
    let mut avg_of = vec![0f32; slots];
    let mut votes_of = vec![0i64; slots];

    for i in 0..nr {
        let tid = ratings_id(&ratings, i) as usize;
        let avg = ratings_avg(&ratings, i);
        let votes = ratings_votes(&ratings, i);
        if tid < slots && avg >= p.min_rating && votes >= p.min_votes {
            avg_of[tid] = avg;
            votes_of[tid] = votes;
            ok_title[tid >> 6] |= 1 << (tid & 63);
        }
    }

    for i in 0..nb {
        let tid = basics_id(&basics, i) as usize;
        let yr = basics_year(&basics, i);
        let fl = basics_flags(&basics, i);
        if tid < slots && (fl & 1 == 0 || yr < p.min_year) {
            ok_title[tid >> 6] &= !(1 << (tid & 63));
        }
    }

    let mut max_nid = 0u64;
    for i in 0..nn {
        max_nid = max_nid.max(names_id(&names, i));
    }
    let nslots = max_nid as usize + 1;
    let mut ok_name = vec![0u64; nslots / 64 + 1];
    let mut idx_of = vec![u32::MAX; nslots];
    for i in 0..nn {
        let nid = names_id(&names, i) as usize;
        let by = names_year(&names, i);
        if nid < nslots {
            idx_of[nid] = i as u32;
            if by >= p.min_birth_year {
                ok_name[nid >> 6] |= 1 << (nid & 63);
            }
        }
    }

    let mut str_off = vec![0u32; nn + 1];
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

    let mut counts = vec![0u32; nslots];
    let mut sum_avg = vec![0.0f64; nslots];
    let mut sum_votes = vec![0i64; nslots];

    for i in 0..ne {
        let nid = u64_at(&edges, i * 2) as usize;
        let tid = u64_at(&edges, i * 2 + 1) as usize;
        if nid < nslots
            && tid < slots
            && (ok_name[nid >> 6] & (1 << (nid & 63)) != 0)
            && (ok_title[tid >> 6] & (1 << (tid & 63)) != 0)
        {
            counts[nid] += 1;
            sum_avg[nid] += avg_of[tid] as f64;
            sum_votes[nid] += votes_of[tid];
        }
    }

    let mut candidates: Vec<(u32, f64, i64, String)> = Vec::new();
    for nid in 0..nslots {
        let cnt = counts[nid];
        if cnt >= p.min_count {
            let k = idx_of[nid] as usize;
            let a = str_off[k] as usize;
            let b = str_off[k + 1] as usize;
            let name_bytes = &nameblob[a..b.saturating_sub(1).max(a)];
            let name = String::from_utf8_lossy(name_bytes).to_string();
            let avg = sum_avg[nid] / cnt as f64;
            candidates.push((cnt, avg, sum_votes[nid], name));
        }
    }

    candidates.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| a.3.cmp(&b.3))
    });

    let mut top10 = Vec::with_capacity(p.limit);
    for (cnt, avg, votes, name) in candidates.into_iter().take(p.limit) {
        top10.push(Q4Row { name, count: cnt, avg, votes });
    }

    let exec_ms = t_exec.elapsed().as_secs_f64() * 1e3;
    Ok((top10, load_ms, exec_ms))
}
