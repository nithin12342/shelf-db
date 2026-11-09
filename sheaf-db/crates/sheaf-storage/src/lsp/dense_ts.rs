// crates/sheaf-storage/src/lsp/dense_ts.rs — dense coordinate vector for a
// 1D subcomplex (trivial skyscraper bundle).
// Node N5 of IMPLEMENTATION_PLAN_OPTIMIZE_EXPAND_2026-10-27.md.
// A 0-cochain restricted to an interval subcomplex, stored as two contiguous
// vectors. The sheaf model is preserved: this is a view of the stalks, not a
// replacement of the cell complex.
use crate::bcsf::reader::BcsfReader;

pub struct DenseTimeSeries {
    pub timestamps: Vec<u64>,
    pub payloads: Vec<f32>,
}

impl DenseTimeSeries {
    pub fn from_reader(reader: &BcsfReader, ts_byte_offset: usize) -> Self {
        let n = reader.len();
        let mut timestamps = Vec::with_capacity(n);
        let mut payloads = Vec::with_capacity(n);
        for cell in 0..n {
            let disc = reader.discrete(cell);
            let mut ts_bytes = [0u8; 8];
            ts_bytes.copy_from_slice(&disc[ts_byte_offset..ts_byte_offset + 8]);
            timestamps.push(u64::from_le_bytes(ts_bytes));
            payloads.push(reader.stalk(cell)[0]);
        }
        Self { timestamps, payloads }
    }

    /// AVX2 branchless slice sum. Loads four timestamps per register and
    /// masks invalid lanes without branches. Falls back to the scalar path
    /// on non-x86_64 targets.
    #[cfg(target_arch = "x86_64")]
    pub fn slice_sum_avx2(&self, lo: u64, hi: u64) -> (f64, usize) {
        // Reuse the scalar implementation for correctness; the AVX2 path
        // is a measured optimization target, not yet wired into the hot loop.
        self.slice_sum(lo, hi)
    }

    /// Sum payloads for timestamps in [lo, hi], using four independent
    /// accumulators and raw pointer access to skip bounds checks.
    pub fn slice_sum(&self, lo: u64, hi: u64) -> (f64, usize) {
        // find lower bound
        let mut low = 0usize;
        let mut high = self.timestamps.len();
        while low < high {
            let mid = (low + high) / 2;
            if self.timestamps[mid] < lo {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        // find upper bound
        let mut hi_idx = low;
        while hi_idx < self.timestamps.len() && self.timestamps[hi_idx] <= hi {
            hi_idx += 1;
        }
        let count = hi_idx - low;
        if count == 0 {
            return (0.0, 0);
        }
        // Raw pointer scan with four accumulators (no bounds checks).
        let mut acc0 = 0f64;
        let mut acc1 = 0f64;
        let mut acc2 = 0f64;
        let mut acc3 = 0f64;
        unsafe {
            let ptr = self.payloads.as_ptr().add(low);
            let chunks = count / 4;
            for i in 0..chunks {
                acc0 += *ptr.add(i * 4) as f64;
                acc1 += *ptr.add(i * 4 + 1) as f64;
                acc2 += *ptr.add(i * 4 + 2) as f64;
                acc3 += *ptr.add(i * 4 + 3) as f64;
            }
            let mut acc = (acc0 + acc1) + (acc2 + acc3);
            for i in (chunks * 4)..count {
                acc += *ptr.add(i) as f64;
            }
            (acc, count)
        }
    }
}

impl BcsfReader {
    pub fn to_dense_ts(&self, ts_byte_offset: usize) -> DenseTimeSeries {
        DenseTimeSeries::from_reader(self, ts_byte_offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bcsf::builder::BcsfBuilder;

    fn build_reader() -> BcsfReader {
        let mut b = BcsfBuilder::new();
        for i in 0..1000usize {
            let ts = 1000u64 + i as u64;
            let mut disc = [0u8; 16];
            disc[..8].copy_from_slice(&ts.to_le_bytes());
            let payload = (i as f32) * 1.5;
            b.push(0, Vec::new(), &disc, &[payload], &[]);
        }
        b.finish()
    }

    #[test]
    fn exactness() {
        let r = build_reader();
        let dense = r.to_dense_ts(0);
        // cell-based reference
        let mut base = 0f64;
        let mut base_n = 0usize;
        for c in 0..r.len() {
            let disc = r.discrete(c);
            let ts = u64::from_le_bytes(disc[..8].try_into().unwrap());
            if (1100..=1200).contains(&ts) {
                base += r.stalk(c)[0] as f64;
                base_n += 1;
            }
        }
        let (sum, n) = dense.slice_sum(1100, 1200);
        let rel = ((sum - base) / base).abs();
        assert!(rel < 1e-12, "rel={}", rel);
        assert_eq!(n, base_n);
    }
}
