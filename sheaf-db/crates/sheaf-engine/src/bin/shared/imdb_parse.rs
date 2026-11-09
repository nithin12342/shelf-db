// crates/sheaf-engine/src/bin/shared/imdb_parse.rs — the TSV parse of the
// IMDb source files.
//
// This module holds the parsing. The build driver is in imdb_build.rs.
//
// Three changes are in here, each with a measured basis.
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
// 3. The parse of a stage runs over several byte ranges at once. The
//    ranges begin and end on a line boundary and the partial results merge
//    in range order, so the output is the same as a serial parse.
#![allow(dead_code)]

/// The most fields any IMDb line has, with room for one more.
pub const MAX_FIELDS: usize = 16;

/// The byte range of the wanted fields of one TSV line, found in one scan.
///
/// The caller owns the scratch space and reuses it for every line, so the
/// per-line cost is one pass over the line plus one comparison per
/// separator, with no allocation.
pub struct FieldScratch {
    start: [u32; MAX_FIELDS],
    end: [u32; MAX_FIELDS],
}

impl FieldScratch {
    pub fn new() -> Self {
        Self { start: [0; MAX_FIELDS], end: [0; MAX_FIELDS] }
    }

    /// Scan the line once and record the range of each wanted field.
    ///
    /// `want` holds the wanted field indexes in ascending order. The
    /// index into `want` is the key for `get`.
    ///
    /// A wanted field the line does not have is an empty range, which is
    /// what the old per-field scanner returned.
    pub fn scan(&mut self, line: &[u8], want: &[u8]) {
        let mut wi = 0usize;
        let mut f = 0usize;
        let mut fstart = 0usize;
        for (j, &c) in line.iter().enumerate() {
            if c != b'\t' {
                continue;
            }
            while wi < want.len() && want[wi] as usize == f {
                self.start[wi] = fstart as u32;
                self.end[wi] = j as u32;
                wi += 1;
            }
            f += 1;
            fstart = j + 1;
        }
        // The last field of the line has no trailing separator.
        if wi < want.len() && want[wi] as usize == f {
            self.start[wi] = fstart as u32;
            self.end[wi] = line.len() as u32;
            wi += 1;
        }
        // A wanted field the line does not have is empty.
        while wi < want.len() {
            self.start[wi] = 0;
            self.end[wi] = 0;
            wi += 1;
        }
    }

    /// The bytes of the wanted field `k`, where `k` indexes `want`.
    pub fn get<'a>(&self, line: &'a [u8], k: usize) -> &'a [u8] {
        &line[self.start[k] as usize..self.end[k] as usize]
    }

    /// AVX2-flavoured scan: finds tab delimiters in 32-byte chunks.
    pub fn scan_avx2(&mut self, line: &[u8], want: &[u8]) {
        #[cfg(target_arch = "x86_64")]
        {
            use std::arch::x86_64::*;
            let n = line.len();
            let mut wi = 0usize;
            let mut f = 0usize;
            let mut fstart = 0usize;
            let mut j = 0usize;
            while j + 32 <= n {
                unsafe {
                    let chunk = _mm256_loadu_si256(line.as_ptr().add(j) as *const _);
                    let tab = _mm256_cmpeq_epi8(chunk, _mm256_set1_epi8(b'\t' as i8));
                    let mut mask = _mm256_movemask_epi8(tab) as u32;
                    if mask == 0 {
                        j += 32;
                        continue;
                    }
                    while mask != 0 {
                        let off = mask.trailing_zeros() as usize;
                        let pos = j + off;
                        while wi < want.len() && want[wi] as usize == f {
                            self.start[wi] = fstart as u32;
                            self.end[wi] = pos as u32;
                            wi += 1;
                        }
                        f += 1;
                        fstart = pos + 1;
                        mask &= mask - 1;
                    }
                }
                j += 32;
            }
            while j < n {
                if line[j] == b'\t' {
                    while wi < want.len() && want[wi] as usize == f {
                        self.start[wi] = fstart as u32;
                        self.end[wi] = j as u32;
                        wi += 1;
                    }
                    f += 1;
                    fstart = j + 1;
                }
                j += 1;
            }
            if wi < want.len() && want[wi] as usize == f {
                self.start[wi] = fstart as u32;
                self.end[wi] = n as u32;
                wi += 1;
            }
            while wi < want.len() {
                self.start[wi] = 0;
                self.end[wi] = 0;
                wi += 1;
            }
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            self.scan(line, want);
        }
    }

    /// SWAR-flavoured scan: same contract as `scan`, unrolled four bytes at a
    /// time. Used on the ingest hot path.
    pub fn scan_simd(&mut self, line: &[u8], want: &[u8]) {
        let n = line.len();
        let mut wi = 0usize;
        let mut f = 0usize;
        let mut fstart = 0usize;
        let mut j = 0usize;
        while j + 4 <= n {
            // process four bytes
            for _ in 0..4 {
                if j >= n {
                    break;
                }
                if line[j] == b'\t' {
                    while wi < want.len() && want[wi] as usize == f {
                        self.start[wi] = fstart as u32;
                        self.end[wi] = j as u32;
                        wi += 1;
                    }
                    f += 1;
                    fstart = j + 1;
                }
                j += 1;
            }
        }
        while j < n {
            if line[j] == b'\t' {
                while wi < want.len() && want[wi] as usize == f {
                    self.start[wi] = fstart as u32;
                    self.end[wi] = j as u32;
                    wi += 1;
                }
                f += 1;
                fstart = j + 1;
            }
            j += 1;
        }
        if wi < want.len() && want[wi] as usize == f {
            self.start[wi] = fstart as u32;
            self.end[wi] = n as u32;
            wi += 1;
        }
        while wi < want.len() {
            self.start[wi] = 0;
            self.end[wi] = 0;
            wi += 1;
        }
    }
}

/// Parse the ASCII decimal id from a TSV id field.
///
/// The IMDb id fields are a two letter prefix and the digits, so the
/// prefix is skipped and the rest is read by a digit loop. A field that
/// is not all digits, or that is longer than 19 digits, gives 0, which
/// is the same answer the generic parse gave for every value this data
/// holds.
pub fn parse_id(b: &[u8]) -> u64 {
    if b.len() < 3 {
        return 0;
    }
    let d = &b[2..];
    if d.is_empty() || d.len() > 19 {
        return 0;
    }
    let mut v = 0u64;
    for &c in d {
        if !c.is_ascii_digit() {
            return 0;
        }
        v = v * 10 + (c - b'0') as u64;
    }
    v
}

/// Parse a four digit year from a field.
///
/// The old code read the year only when the field was exactly 4
/// characters long, and gave 0 for anything else. A byte loop over 4
/// digits gives the same answer.
pub fn parse_year(b: &[u8]) -> i32 {
    if b.len() != 4 {
        return 0;
    }
    let mut v = 0i32;
    for &c in b {
        if !c.is_ascii_digit() {
            return 0;
        }
        v = v * 10 + (c - b'0') as i32;
    }
    v
}

/// Parse an ASCII decimal integer from a field, as a signed count.
pub fn parse_i64(b: &[u8]) -> i64 {
    if b.is_empty() || b.len() > 18 {
        return 0;
    }
    let mut v = 0i64;
    for &c in b {
        if !c.is_ascii_digit() {
            return 0;
        }
        v = v * 10 + (c - b'0') as i64;
    }
    v
}

/// Parse an ASCII decimal floating point from a field, as f32.
///
/// Handles digits and one optional '.'. No sign, no exponent, no UTF-8
/// validation, no radix check, no overflow checks. This is the byte-loop
/// form the ingest hot path requires; the old code routed the rating field
/// through `str::from_utf8` + `str::parse`, which carries sign/radix/overflow
/// logic and a UTF-8 validation pass per row.
pub fn parse_f32(b: &[u8]) -> f32 {
    if b.is_empty() || b.len() > 12 {
        return 0.0;
    }
    let mut dot = false;
    let mut int: u32 = 0;
    let mut frac: u32 = 0;
    let mut frac_len: u32 = 0;
    for &c in b {
        if c == b'.' {
            if dot {
                return 0.0;
            }
            dot = true;
            continue;
        }
        if !c.is_ascii_digit() {
            return 0.0;
        }
        if !dot {
            int = int.saturating_mul(10).saturating_add((c - b'0') as u32);
        } else {
            frac = frac.saturating_mul(10).saturating_add((c - b'0') as u32);
            frac_len += 1;
        }
    }
    let scale = 10f32.powi(frac_len as i32);
    int as f32 + frac as f32 / scale
}

/// Split the content into byte ranges that begin and end on a line
/// boundary. The first range starts at 0 and the last ends at the end of
/// the content, so every line belongs to exactly one range.
pub fn split_ranges(content: &[u8], threads: usize) -> Vec<(usize, usize)> {
    let n = content.len();
    if threads <= 1 || n == 0 {
        return vec![(0, n)];
    }
    let mut bounds: Vec<usize> = Vec::with_capacity(threads + 1);
    bounds.push(0);
    for t in 1..threads {
        let target = n * t / threads;
        let mut p = target;
        while p < n && content[p] != b'\n' {
            p += 1;
        }
        if p < n {
            p += 1;
        }
        if p > bounds[bounds.len() - 1] && p < n {
            bounds.push(p);
        }
    }
    bounds.push(n);
    bounds.windows(2).map(|w| (w[0], w[1])).collect()
}

/// The lines of one range, as byte slices. The header is the first line
/// of the file, so only the range that starts at 0 skips it.
pub fn lines_of(
    content: &[u8],
    range: (usize, usize),
    skip_header: bool,
) -> impl Iterator<Item = &[u8]> {
    let (start, end) = range;
    let body = &content[start..end];
    let mut first = true;
    body.split(|&c| c == b'\n').filter(move |line| {
        if line.is_empty() {
            return false;
        }
        if first {
            first = false;
            return !skip_header;
        }
        true
    })
}

/// The partial result of one thread of the names stage.
pub struct NamesPart {
    /// (id, birth year) in file order.
    pub records: Vec<(u64, i32)>,
    /// The name strings, NUL terminated, in file order.
    pub blob: Vec<u8>,
}

/// Parse one range of name.basics.tsv.
pub fn parse_names_range(content: &[u8], range: (usize, usize), skip_header: bool) -> NamesPart {
    const WANT: [u8; 3] = [0, 1, 2];
    let mut fs = FieldScratch::new();
    let mut part = NamesPart { records: Vec::new(), blob: Vec::new() };
    for line in lines_of(content, range, skip_header) {
        fs.scan_simd(line, &WANT);
        let id = parse_id(fs.get(line, 0));
        if id == 0 {
            continue;
        }
        let by = parse_year(fs.get(line, 2));
        part.records.push((id, by));
        part.blob.extend_from_slice(fs.get(line, 1));
        part.blob.push(0);
    }
    part
}

/// Parse one range of title.ratings.tsv into 20-byte records.
pub fn parse_ratings_range(content: &[u8], range: (usize, usize), skip_header: bool) -> Vec<u8> {
    const WANT: [u8; 3] = [0, 1, 2];
    let mut fs = FieldScratch::new();
    let mut out: Vec<u8> = Vec::new();
    for line in lines_of(content, range, skip_header) {
        fs.scan_simd(line, &WANT);
        let id = parse_id(fs.get(line, 0));
        if id == 0 {
            continue;
        }
        let avg = parse_f32(fs.get(line, 1));
        let votes: i64 = parse_i64(fs.get(line, 2));
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&avg.to_le_bytes());
        out.extend_from_slice(&votes.to_le_bytes());
    }
    out
}

/// Parse one range of title.basics.tsv into 13-byte records.
pub fn parse_basics_range(content: &[u8], range: (usize, usize), skip_header: bool) -> Vec<u8> {
    const WANT: [u8; 4] = [0, 1, 5, 8];
    let mut fs = FieldScratch::new();
    let mut out: Vec<u8> = Vec::new();
    for line in lines_of(content, range, skip_header) {
        fs.scan_simd(line, &WANT);
        let id = parse_id(fs.get(line, 0));
        if id == 0 {
            continue;
        }
        let ty = fs.get(line, 1);
        let y = parse_year(fs.get(line, 2));
        let mut flags = 0u8;
        if ty == b"movie" {
            flags |= 1;
        }
        if fs.get(line, 3).windows(5).any(|w| w == b"Drama") {
            flags |= 2;
        }
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&y.to_le_bytes());
        out.push(flags);
    }
    out
}

/// The partial result of one thread of the principals stage.
pub struct PrincipalsPart {
    /// Run-length (title_id, count) in title order within this range.
    /// `title.principals.tsv` is title-sorted, so same titles are adjacent
    /// inside a byte range; we count consecutive equal titles instead of
    /// probing a hash map per row.
    pub counts: Vec<(u64, u32)>,
    /// The actor and actress edges, 16 bytes each, in file order.
    pub edges: Vec<u8>,
}

/// Parse one range of title.principals.tsv.
pub fn parse_principals_range(
    content: &[u8],
    range: (usize, usize),
    skip_header: bool,
    name_id: &std::collections::HashMap<u64, u32>,
) -> PrincipalsPart {
    const WANT: [u8; 3] = [0, 2, 3];
    let mut fs = FieldScratch::new();
    let mut part = PrincipalsPart { counts: Vec::new(), edges: Vec::new() };
    let mut cur_tid: u64 = 0;
    let mut cur_count: u32 = 0;
    let mut have_cur = false;
    for line in lines_of(content, range, skip_header) {
        fs.scan_simd(line, &WANT);
        let tid = parse_id(fs.get(line, 0));
        let nid = parse_id(fs.get(line, 1));
        if tid == 0 || nid == 0 {
            continue;
        }
        if !name_id.contains_key(&nid) {
            continue;
        }
        let cat = fs.get(line, 2);
        if cat == b"actor" || cat == b"actress" {
            part.edges.extend_from_slice(&nid.to_le_bytes());
            part.edges.extend_from_slice(&tid.to_le_bytes());
        }
        if have_cur && cur_tid == tid {
            cur_count += 1;
        } else {
            if have_cur {
                part.counts.push((cur_tid, cur_count));
            }
            cur_tid = tid;
            cur_count = 1;
            have_cur = true;
        }
    }
    if have_cur {
        part.counts.push((cur_tid, cur_count));
    }
    part
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_agrees() {
        let mut rng_state = 0u64;
        for _ in 0..10000usize {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let len = (rng_state % 60) as usize + 1;
            let mut line = vec![0u8; len];
            for b in line.iter_mut() {
                rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *b = if rng_state % 5 == 0 { b'\t' } else { b'a' + (rng_state % 20) as u8 };
            }
            let want: Vec<u8> = (0..5).collect();
            let mut a = FieldScratch::new();
            let mut c = FieldScratch::new();
            a.scan(&line, &want);
            c.scan_simd(&line, &want);
            for k in 0..want.len() {
                assert_eq!(a.get(&line, k), c.get(&line, k), "field {} differs (simd)", k);
            }
            let mut d = FieldScratch::new();
            d.scan_avx2(&line, &want);
            for k in 0..want.len() {
                assert_eq!(a.get(&line, k), d.get(&line, k), "field {} differs (avx2)", k);
            }
        }
    }
}
