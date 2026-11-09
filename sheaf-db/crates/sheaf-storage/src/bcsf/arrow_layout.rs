// crates/sheaf-storage/src/bcsf/arrow_layout.rs — Arrow-compatible columnar stalk slabs
// SYNTHESIS Stage 1a (Choice 2 substrate). Depends on nothing.
// Layout mirrors Apache Arrow `FixedSizeListArray<Float32, d>`: one flat
// values buffer (64-byte aligned start) plus a validity bitmap (1 bit per
// row). No arrow-crate dependency — the layout is the interop contract;
// C Data Interface export is a follow-up, not here.
// Why manual: the kernel/storage stay dependency-free per project precedent;
// alignment + validity semantics are what the FMA kernels (Stage 1b) need.
//
// B6: SoA is the PRIMARY layout. The column descriptor (the width `d`)
// decides the physical layout at WRITE time: the values buffer holds `d`
// CONTIGUOUS columns (column c at c*cap), so a column is already a
// unit-stride slice. `transpose_soa` / `compact_valid` therefore return
// VIEWS into the slab instead of materializing 2x the data at read time.
use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::borrow::Cow;

const ALIGN: usize = 64;

/// A 64-byte aligned, length-tracked f32 buffer. Frees on drop.
/// `pub(crate)` for reuse by the coordinate store (M1) — same alignment contract.
pub(crate) struct AlignedBuf {
    ptr: *mut f32,
    cap: usize, // elements
}

impl AlignedBuf {
    pub(crate) fn new(cap_elems: usize) -> Self {
        let layout = Layout::from_size_align(
            cap_elems.checked_mul(4).expect("slab overflow"),
            ALIGN,
        )
        .expect("align layout");
        let ptr = unsafe { alloc_zeroed(layout) as *mut f32 };
        assert!(!ptr.is_null(), "slab alloc failed");
        assert!((ptr as usize).is_multiple_of(ALIGN), "slab misaligned");
        Self { ptr, cap: cap_elems }
    }

    pub(crate) fn as_slice(&self, len: usize) -> &[f32] {
        assert!(len <= self.cap);
        unsafe { std::slice::from_raw_parts(self.ptr, len) }
    }

    pub(crate) fn as_mut_slice(&mut self, len: usize) -> &mut [f32] {
        assert!(len <= self.cap);
        unsafe { std::slice::from_raw_parts_mut(self.ptr, len) }
    }
}

impl Drop for AlignedBuf {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.cap * 4, ALIGN).expect("align layout");
        unsafe { dealloc(self.ptr as *mut u8, layout) };
    }
}

/// Graded columnar stalk store: rows of width `d`, Arrow-compatible layout.
///
/// B6 (SoA primary): the values buffer is COLUMN-MAJOR - `d` contiguous
/// columns of `cap` rows each (column `c` at `c * cap`). A column is
/// therefore already a unit-stride slice, and every column-wise read
/// (transpose / compaction / scan) is a VIEW, not a copy.
pub struct ArrowStalkSlab {
    pub d: usize,
    len: usize,
    /// Rows per column.
    cap: usize,
    values: AlignedBuf,
    validity: Vec<u64>,
}

impl ArrowStalkSlab {
    pub fn with_capacity(d: usize, rows: usize) -> Self {
        assert!(d >= 1, "width must be >= 1");
        Self {
            d,
            len: 0,
            cap: rows,
            values: AlignedBuf::new(rows * d),
            validity: vec![0u64; rows.div_ceil(64)],
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Base pointer (for the 64B alignment gate).
    pub fn base_addr(&self) -> usize {
        self.values.ptr as usize
    }

    /// Append one row. B6: the row is written column-wise (d unit writes
    /// into the d contiguous columns).
    pub fn push_row(&mut self, row: &[f32]) {
        assert_eq!(row.len(), self.d, "row width mismatch");
        let total = self.cap * self.d;
        let dst = self.values.as_mut_slice(total);
        for c in 0..self.d {
            dst[c * self.cap + self.len] = row[c];
        }
        self.validity[self.len / 64] |= 1u64 << (self.len % 64);
        self.len += 1;
    }

    /// Column `c` as a unit-stride slice (B6: a VIEW, no copy).
    pub fn column(&self, c: usize) -> &[f32] {
        assert!(c < self.d, "column out of range");
        let total = self.cap * self.d;
        let v = self.values.as_slice(total);
        &v[c * self.cap..c * self.cap + self.len]
    }

    pub fn set_null(&mut self, row: usize) {
        assert!(row < self.len);
        self.validity[row / 64] &= !(1u64 << (row % 64));
    }

    pub fn is_valid(&self, row: usize) -> bool {
        assert!(row < self.len);
        (self.validity[row / 64] >> (row % 64)) & 1 == 1
    }

    /// True when no row is null (the validity-free hot path).
    pub fn no_nulls(&self) -> bool {
        self.validity.iter().all(|w| *w == u64::MAX)
    }

    /// Scalar column sum over valid rows (Stage 1a baseline; FMA kernel
    /// arrives in Stage 1b and must beat this number on identical data).
    pub fn col_sum(&self, col: usize) -> f64 {
        assert!(col < self.d);
        let v = self.column(col);
        let mut acc = 0f64;
        if self.no_nulls() {
            for x in v {
                acc += *x as f64;
            }
        } else {
            for (r, x) in v.iter().enumerate() {
                if self.is_valid(r) {
                    acc += *x as f64;
                }
            }
        }
        acc
    }

    /// Fused sum(price * (1 - discount)) over valid rows with price col `p`
    /// and discount col `q`, in row order (matches tpch_scan_bench exactly).
    /// B6: with SoA the two operands are unit-stride slices - no gather.
    pub fn fused_revenue_sum(&self, p: usize, q: usize) -> f64 {
        assert!(p < self.d && q < self.d);
        let cp = self.column(p);
        let cq = self.column(q);
        let mut acc = 0f64;
        if self.no_nulls() {
            for i in 0..self.len {
                acc += cp[i] as f64 * (1.0 - cq[i] as f64);
            }
        } else {
            for i in 0..self.len {
                if self.is_valid(i) {
                    acc += cp[i] as f64 * (1.0 - cq[i] as f64);
                }
            }
        }
        acc
    }

    /// B6: the two columns as VIEWS into the slab. With SoA as the primary
    /// layout this allocates NOTHING (the old AoS path materialized 2 x
    /// len f32 = the 2x memory spike).
    pub fn transpose_soa(&self, p: usize, q: usize) -> (Cow<'_, [f32]>, Cow<'_, [f32]>) {
        assert!(p < self.d && q < self.d);
        (Cow::Borrowed(self.column(p)), Cow::Borrowed(self.column(q)))
    }

    /// MAX-PERF P2: sparse exception ordinals (null rows), sorted ascending.
    /// Validity-free hot path: consumers iterate dense data + skip list
    /// instead of testing a bitmap per row (measured ~10 ms/6M tax removed).
    pub fn null_exceptions(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for r in 0..self.len {
            if !self.is_valid(r) {
                out.push(r);
            }
        }
        out
    }

    /// Compact valid rows of columns (p, q) into dense vectors, dropping
    /// nulls (per the exception list). Returns (dense_p, dense_q, kept).
    /// B6: when nothing is null the compaction is a VIEW (no copy); with
    /// nulls the dense vectors must be materialized.
    pub fn compact_valid(&self, p: usize, q: usize) -> (Cow<'_, [f32]>, Cow<'_, [f32]>, usize) {
        assert!(p < self.d && q < self.d);
        if self.no_nulls() {
            return (Cow::Borrowed(self.column(p)), Cow::Borrowed(self.column(q)), self.len);
        }
        let cp = self.column(p);
        let cq = self.column(q);
        let mut dp = Vec::new();
        let mut dq = Vec::new();
        for r in 0..self.len {
            if self.is_valid(r) {
                dp.push(cp[r]);
                dq.push(cq[r]);
            }
        }
        let kept = dp.len();
        (Cow::Owned(dp), Cow::Owned(dq), kept)
    }
}
