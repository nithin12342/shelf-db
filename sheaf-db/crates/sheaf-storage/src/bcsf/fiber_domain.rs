// crates/sheaf-storage/src/bcsf/fiber_domain.rs — per-domain incremental
// fiber update (N4 / METHOD-033, REQ-038 / SPEC-038).
//
// A fiber is a sorted key -> cell index. Re-sorting the WHOLE index after
// every write is O(n log n) per write; re-sorting it once per COMPACTION
// is O(n log n) per compaction. This splits the fiber into ~64 MB domains
// (key ranges) and re-sorts ONLY the domains that received writes.
//
// The gate is the RE-SORT COUNT: 1,000 writes must cost as many re-sorts
// as there are dirty domains - never 1,000.

use rayon::prelude::*;

/// One domain of a fiber: a contiguous key range with its sorted entries.
#[derive(Clone, Debug, Default)]
pub struct FiberDomain {
    /// First key in the domain (inclusive).
    pub key_lo: u64,
    /// Last key in the domain (exclusive).
    pub key_hi: u64,
    /// Sorted (key, cell) pairs.
    pub sorted: Vec<(u64, u32)>,
    /// Writes received since the last flush (not yet merged).
    pub pending: Vec<(u64, u32)>,
    /// Whether the domain needs a re-sort.
    pub dirty: bool,
}

impl FiberDomain {
    /// Binary search the merged view (pending writes included).
    pub fn lookup(&self, key: u64) -> Option<u32> {
        // pending writes shadow the sorted entries (last write wins); the
        // pending list is not sorted, so it is scanned in reverse.
        for (k, v) in self.pending.iter().rev() {
            if *k == key {
                return Some(*v);
            }
        }
        match self.sorted.binary_search_by(|probe| probe.0.cmp(&key)) {
            Ok(i) => Some(self.sorted[i].1),
            Err(_) => None,
        }
    }
}

/// A fiber split into domains, with incremental updates.
#[derive(Clone, Debug, Default)]
pub struct FiberDomains {
    pub domains: Vec<FiberDomain>,
    /// Number of domains re-sorted so far.
    pub resort_count: u64,
    /// Number of writes applied.
    pub write_count: u64,
    /// Number of flushes.
    pub flush_count: u64,
}

impl FiberDomains {
    /// Build a fiber from (key, cell) pairs, split into domains of at most
    /// `domain_bytes` estimated bytes (16 B per entry).
    pub fn build(entries: &[(u64, u32)], domain_bytes: usize) -> FiberDomains {
        let per_domain = (domain_bytes / 16).max(1);
        let mut sorted: Vec<(u64, u32)> = entries.to_vec();
        sorted.par_sort_unstable();
        let mut domains: Vec<FiberDomain> = Vec::new();
        let mut start = 0usize;
        while start < sorted.len() {
            let end = (start + per_domain).min(sorted.len());
            let key_lo = sorted[start].0;
            let key_hi = if end < sorted.len() { sorted[end].0 } else { u64::MAX };
            domains.push(FiberDomain {
                key_lo,
                key_hi,
                sorted: sorted[start..end].to_vec(),
                pending: Vec::new(),
                dirty: false,
            });
            start = end;
        }
        if domains.is_empty() {
            domains.push(FiberDomain { key_lo: 0, key_hi: u64::MAX, ..Default::default() });
        }
        FiberDomains { domains, ..Default::default() }
    }

    /// Index of the domain that owns `key`.
    pub fn domain_of(&self, key: u64) -> usize {
        // domains are ordered by key range
        let mut lo = 0usize;
        let mut hi = self.domains.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            let d = &self.domains[mid];
            if key < d.key_lo {
                hi = mid;
            } else if key >= d.key_hi {
                lo = mid + 1;
            } else {
                return mid;
            }
        }
        self.domains.len() - 1
    }

    /// Apply one write; the owning domain becomes dirty.
    pub fn write(&mut self, key: u64, cell: u32) {
        let idx = self.domain_of(key);
        let d = &mut self.domains[idx];
        d.pending.push((key, cell));
        d.dirty = true;
        self.write_count += 1;
    }

    /// Merge + re-sort ONLY the dirty domains. Returns the re-sort count.
    pub fn flush(&mut self) -> u64 {
        let mut resorts = 0u64;
        for d in self.domains.iter_mut() {
            if !d.dirty {
                continue;
            }
            // merge the pending writes into the sorted entries
            let mut merged: Vec<(u64, u32)> = Vec::with_capacity(d.sorted.len() + d.pending.len());
            let mut i = 0usize;
            let mut j = 0usize;
            while i < d.sorted.len() && j < d.pending.len() {
                if d.sorted[i].0 <= d.pending[j].0 {
                    merged.push(d.sorted[i]);
                    i += 1;
                } else {
                    merged.push(d.pending[j]);
                    j += 1;
                }
            }
            merged.extend_from_slice(&d.sorted[i..]);
            merged.extend_from_slice(&d.pending[j..]);
            // last write wins on equal keys: a STABLE sort keeps the
            // pending (newer) entry after the sorted (older) one.
            merged.sort_by_key(|e| e.0);
            merged.dedup_by(|a, b| {
                if a.0 == b.0 {
                    b.1 = a.1;
                    true
                } else {
                    false
                }
            });
            d.sorted = merged;
            d.pending.clear();
            d.dirty = false;
            resorts += 1;
        }
        self.resort_count += resorts;
        self.flush_count += 1;
        resorts
    }

    /// Fiber answer: the cell for `key` (index lookup).
    pub fn lookup(&self, key: u64) -> Option<u32> {
        self.domains[self.domain_of(key)].lookup(key)
    }

    /// FALLBACK answer: a full scan of the merged view.
    pub fn scan_lookup(&self, key: u64) -> Option<u32> {
        for d in &self.domains {
            // pending writes shadow the sorted entries
            for (k, v) in d.pending.iter().rev() {
                if *k == key {
                    return Some(*v);
                }
            }
            if let Ok(i) = d.sorted.binary_search_by(|probe| probe.0.cmp(&key)) {
                return Some(d.sorted[i].1);
            }
        }
        None
    }

    /// Number of domains currently dirty.
    pub fn dirty_domains(&self) -> usize {
        self.domains.iter().filter(|d| d.dirty).count()
    }

    /// The aggregate the fiber answers: sum of all cells (pending included).
    pub fn aggregate(&self) -> u64 {
        let mut sum: u64 = 0;
        for d in &self.domains {
            for (_, v) in &d.sorted {
                sum += *v as u64;
            }
            for (_, v) in &d.pending {
                sum += *v as u64;
            }
        }
        sum
    }
}
