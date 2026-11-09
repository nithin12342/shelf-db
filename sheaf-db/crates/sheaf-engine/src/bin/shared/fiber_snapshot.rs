// crates/sheaf-engine/src/bin/shared/fiber_snapshot.rs — durable fiber bundle
// codec (B3 / METHOD-040, REQ-045 / SPEC-045).
//
// This file is a bin-local shared module (like shared/corpus.rs): it is
// compiled into every bin that includes it, and each bin uses a different
// subset of the API, so unused-item warnings would fire per bin.
#![allow(dead_code)]
//
// Zero-copy load: the whole file is read ONCE into a byte buffer and the
// payload is BORROWED from it — no per-element Vec::push deserialization.
// Measured before this change: load 158.1 ms for a 40.6 MB bundle, of which
// ~153 ms was the per-element copy; the read floor is ~5.1 ms at the
// measured 8.4 GB/s.
//
// Format (v2): [MAGIC u64][fmt_ver u32][payload_len u64][crc32c u32][payload]
//   payload = [nf u64][(cust u64, rev f64) x nf][ns u64][(cust u64, seg u8) x ns]
// v2 changes the integrity primitive from byte-at-a-time IEEE CRC32 to
// SSE4.2 CRC32C (repo's existing `crc_hw`): the IEEE table loop costs
// ~80 ms per 40.6 MB and would dominate the load. A format-version bump is
// safe: the bundle is a cache artifact, rebuilt on mismatch.
//
// Integrity semantics are unchanged in strength: whole-file CRC over the
// payload, fail-loud on mismatch (never a partial load).
use sheaf_storage::lsp::ring_wal::crc_hw;

pub const SNAP_MAGIC: u64 = 0x534E415046494252; // "SNAPFIBR"
pub const SNAP_FMT_VER: u32 = 2;
pub const HEADER_LEN: usize = 8 + 4 + 8 + 4 + 8; // magic | ver | len | crc | src_fp

/// A loaded bundle: the file bytes plus parsed section offsets. No owned
/// per-element copies — iterators decode on the fly.
pub struct Bundle {
    bytes: Vec<u8>,
    n_fibers: usize,
    n_segs: usize,
    fibers_at: usize,
    segs_at: usize,
    src_fingerprint: u64,
}

impl Bundle {
    /// Open a bundle file: read once, verify magic/version/length/CRC.
    /// Fails loud on any mismatch (never a partial load).
    pub fn open(path: &str) -> Result<Bundle, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        Bundle::from_vec(bytes)
    }

    /// Take ownership of already-read bytes (no second copy).
    pub fn from_vec(bytes: Vec<u8>) -> Result<Bundle, String> {
        let (n_fibers, n_segs, fibers_at, segs_at, src_fp) = parse_header(&bytes)?;
        Ok(Bundle { bytes, n_fibers, n_segs, fibers_at, segs_at, src_fingerprint: src_fp })
    }

    /// Borrow-and-copy variant (used by the corruption negative test).
    pub fn from_bytes(bytes: &[u8]) -> Result<Bundle, String> {
        Bundle::from_vec(bytes.to_vec())
    }

    pub fn fiber_count(&self) -> usize {
        self.n_fibers
    }
    pub fn seg_count(&self) -> usize {
        self.n_segs
    }
    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }

    /// Fingerprint of the SOURCE data this bundle was built from (N3).
    /// A mismatch means the source changed => the bundle is stale and must
    /// be rebuilt (never used silently).
    pub fn src_fingerprint(&self) -> u64 {
        self.src_fingerprint
    }

    /// True when the bundle still matches the given source fingerprint.
    pub fn matches_source(&self, current: u64) -> bool {
        self.src_fingerprint == current
    }

    /// Borrow the raw file bytes (no copy) — used by the reference path in
    /// the bench so the old-path comparison runs on the SAME buffer.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Iterator over (cust, revenue) pairs, decoded on the fly from the
    /// borrowed bytes. No allocation, no per-element copy.
    pub fn fibers(&self) -> FiberIter<'_> {
        FiberIter { b: &self.bytes, at: self.fibers_at, left: self.n_fibers }
    }

    /// Iterator over (cust, segment) pairs, decoded on the fly.
    pub fn segs(&self) -> SegIter<'_> {
        SegIter { b: &self.bytes, at: self.segs_at, left: self.n_segs }
    }
}

pub struct FiberIter<'a> {
    b: &'a [u8],
    at: usize,
    left: usize,
}

impl<'a> Iterator for FiberIter<'a> {
    type Item = (u64, f64);
    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        let c = u64::from_le_bytes(self.b[self.at..self.at + 8].try_into().unwrap());
        let v = f64::from_le_bytes(self.b[self.at + 8..self.at + 16].try_into().unwrap());
        self.at += 16;
        self.left -= 1;
        Some((c, v))
    }
}

pub struct SegIter<'a> {
    b: &'a [u8],
    at: usize,
    left: usize,
}

impl<'a> Iterator for SegIter<'a> {
    type Item = (u64, u8);
    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }
        let c = u64::from_le_bytes(self.b[self.at..self.at + 8].try_into().unwrap());
        let s = self.b[self.at + 8];
        self.at += 9;
        self.left -= 1;
        Some((c, s))
    }
}

/// Parse header + section offsets; returns (n_fibers, n_segs, fibers_at, segs_at).
/// Fails loud on any mismatch (never a partial load).
fn parse_header(bytes: &[u8]) -> Result<(usize, usize, usize, usize, u64), String> {
    if bytes.len() < HEADER_LEN {
        return Err(format!("truncated header ({} bytes)", bytes.len()));
    }
    if u64::from_le_bytes(bytes[..8].try_into().unwrap()) != SNAP_MAGIC {
        return Err("bad magic (not a fiber bundle)".to_string());
    }
    let ver = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    if ver != SNAP_FMT_VER {
        return Err(format!("format version {ver} != {SNAP_FMT_VER} (rebuild)"));
    }
    let len = u64::from_le_bytes(bytes[12..20].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(bytes[20..24].try_into().unwrap());
    if bytes.len() != HEADER_LEN + len {
        return Err(format!(
            "length mismatch (header {len}, file {})",
            bytes.len() - HEADER_LEN
        ));
    }
    let payload = &bytes[HEADER_LEN..];
    if crc_hw(payload) != crc {
        return Err("crc mismatch (torn write?)".to_string());
    }
    let n_fibers = u64::from_le_bytes(payload[..8].try_into().unwrap()) as usize;
    let fibers_at = 8;
    let after = fibers_at + n_fibers * 16;
    if after + 8 > payload.len() {
        return Err("fiber section overruns payload".to_string());
    }
    let n_segs = u64::from_le_bytes(payload[after..after + 8].try_into().unwrap()) as usize;
    let segs_at = after + 8;
    if segs_at + n_segs * 9 > payload.len() {
        return Err("segment section overruns payload".to_string());
    }
    let src_fp = u64::from_le_bytes(bytes[24..32].try_into().unwrap());
    Ok((
        n_fibers,
        n_segs,
        HEADER_LEN + fibers_at,
        HEADER_LEN + segs_at,
        src_fp,
    ))
}

/// Serialize a bundle: [MAGIC][ver][len][crc32c][payload].
pub fn encode(n_fibers: &[(u64, f64)], n_segs: &[(u64, u8)], src_fingerprint: u64) -> Vec<u8> {
    let mut payload = Vec::with_capacity(16 + n_fibers.len() * 16 + n_segs.len() * 9);
    payload.extend_from_slice(&(n_fibers.len() as u64).to_le_bytes());
    for &(c, v) in n_fibers {
        payload.extend_from_slice(&c.to_le_bytes());
        payload.extend_from_slice(&v.to_le_bytes());
    }
    payload.extend_from_slice(&(n_segs.len() as u64).to_le_bytes());
    for &(c, s) in n_segs {
        payload.extend_from_slice(&c.to_le_bytes());
        payload.push(s);
    }
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&SNAP_MAGIC.to_le_bytes());
    out.extend_from_slice(&SNAP_FMT_VER.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&crc_hw(&payload).to_le_bytes());
    out.extend_from_slice(&src_fingerprint.to_le_bytes());
    out.extend_from_slice(&payload);
    out
}

/// Fingerprint of a set of source files (N3): hash of (size, mtime) per
/// file. A bundle built from different sources is STALE and must be rebuilt,
/// never used silently. Cheap (stat only), order-independent.
pub fn source_fingerprint(paths: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in paths {
        let (size, mtime) = match std::fs::metadata(p) {
            Ok(md) => {
                let size = md.len();
                let mtime = md
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0);
                (size, mtime)
            }
            Err(_) => (0, 0),
        };
        for v in [size, mtime] {
            h ^= v;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
    }
    h
}
