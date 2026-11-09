// crates/sheaf-storage/src/bcsf/coord.rs — dense 0-cochain coordinate vectors
// MATH-UPGRADES Stage M1. Depends on arrow_layout (AlignedBuf reuse).
// Physical form of C^0(P; F_a) ≅ R^|P_0|: one contiguous 64B-aligned f32
// vector per scalar attribute, row = cell ordinal. Aggregation is an inner
// product with the dual identity vector. Append-only; row count fixed at
// seal (compaction rebuilds; no in-place deletes — tombstones live in BCSF).
use super::arrow_layout::AlignedBuf;

/// One attribute's coordinate vector: v_a[i] = F_a(cell i).
pub struct CoordVec {
    buf: AlignedBuf,
    len: usize,
}

impl CoordVec {
    pub fn with_capacity(rows: usize) -> Self {
        Self { buf: AlignedBuf::new(rows.max(1)), len: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn base_addr(&self) -> usize {
        self.buf.as_slice(self.len).as_ptr() as usize
    }

    pub fn push(&mut self, v: f32) {
        let dst = self.buf.as_mut_slice(self.len + 1);
        dst[self.len] = v;
        self.len += 1;
    }

    pub fn as_slice(&self) -> &[f32] {
        self.buf.as_slice(self.len)
    }

    /// Inner product <1, v> = sum. Single timed rep at call site (see
    /// methodology finding: no in-process best-of-N for pure loops).
    pub fn inner_sum(&self) -> f64 {
        let mut acc = 0f64;
        for &x in self.as_slice().iter() {
            acc += x as f64;
        }
        acc
    }
}

/// Fused revenue over two plain slices (unit stride). Free function so the
/// timed loop has no method-call or validity overhead.
pub fn fused_revenue(p: &[f32], q: &[f32]) -> f64 {
    assert_eq!(p.len(), q.len());
    let mut acc = 0f64;
    for (a, b) in p.iter().zip(q.iter()) {
        acc += *a as f64 * (1.0 - *b as f64);
    }
    acc
}
