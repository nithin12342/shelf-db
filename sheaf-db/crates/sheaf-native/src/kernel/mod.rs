//! kernel - the Tier 1 port and its shared vocabulary types.
//!
//! KernelBackend is the interface every consumer depends on (dependency
//! inversion). Two adapters implement it:
//!   - native::NativeKernel   - the C++23 SIMD library, when linked.
//!   - fallback::FallbackKernel - portable scalar Rust, always present.
//!
//! ComplexSpec borrows the topology arrays for one call. It carries no
//! ownership, so the engine can keep its BCSF arenas immutable while the
//! kernel reads them (the affine-lease rule from spec Pillar 4).

pub mod fallback;
#[cfg(sheaf_native_kernel)]
pub mod native;

use std::fmt;

/// Errors shared by both adapters. They mirror the C ABI error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelError {
    /// A null-equivalent buffer, a bad dimension, or a length mismatch.
    InvalidArgument,
    /// The caller scratch arena is smaller than the operation needs.
    ScratchTooSmall,
    /// The iterative solver exhausted max_iters above the tolerance.
    NoConvergence,
}

impl fmt::Display for KernelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument => write!(f, "invalid argument or buffer length"),
            Self::ScratchTooSmall => write!(f, "scratch arena too small"),
            Self::NoConvergence => write!(f, "solver did not converge in max_iters"),
        }
    }
}

impl std::error::Error for KernelError {}

/// Borrowed descriptor of a cellular sheaf with uniform stalk dimension d.
///
/// edge_src and edge_dst list the oriented 1-cells. r_src/r_dst hold row-major
/// d x d restriction blocks per edge, or None for identity restrictions.
#[derive(Clone, Copy)]
pub struct ComplexSpec<'a> {
    pub num_vertices: u32,
    pub stalk_dim: u32,
    pub edge_src: &'a [u32],
    pub edge_dst: &'a [u32],
    pub r_src: Option<&'a [f32]>,
    pub r_dst: Option<&'a [f32]>,
}

impl<'a> ComplexSpec<'a> {
    pub fn num_edges(&self) -> u32 {
        self.edge_src.len() as u32
    }

    /// floats in one full 0-cochain (vertex stalk state).
    pub fn vertex_floats(&self) -> usize {
        self.num_vertices as usize * self.stalk_dim as usize
    }

    /// floats in one full 1-cochain (edge stalk state).
    pub fn edge_floats(&self) -> usize {
        self.num_edges() as usize * self.stalk_dim as usize
    }

    /// Make sure that the descriptor is internally consistent before any
    /// adapter touches a pointer. Both adapters call this first.
    pub fn validate(&self) -> Result<(), KernelError> {
        if self.stalk_dim == 0 || self.edge_src.len() != self.edge_dst.len() {
            return Err(KernelError::InvalidArgument);
        }
        let blocks = self.edge_floats() * self.stalk_dim as usize;
        match (self.r_src, self.r_dst) {
            (None, None) => Ok(()),
            (Some(a), Some(b)) if a.len() == blocks && b.len() == blocks => Ok(()),
            _ => Err(KernelError::InvalidArgument),
        }
    }
}

/// The Tier 1 port. Every method is a pure function over caller buffers.
/// Adapters must be thread-safe so core-pinned workers can share one.
pub trait KernelBackend: Send + Sync {
    /// Human-readable adapter name, used in logs and parity reports.
    fn name(&self) -> &'static str;

    /// Pass 1: edge_out = delta s.
    fn coboundary(
        &self, cx: &ComplexSpec<'_>, s: &[f32], edge_out: &mut [f32],
    ) -> Result<(), KernelError>;

    /// Pass 2: vertex_out = delta* edge_vec (output zeroed first).
    fn coboundary_adjoint(
        &self, cx: &ComplexSpec<'_>, edge_vec: &[f32], vertex_out: &mut [f32],
    ) -> Result<(), KernelError>;

    /// Factored L s = delta* (delta s). scratch_edges holds edge_floats().
    fn laplacian_apply(
        &self, cx: &ComplexSpec<'_>, s: &[f32], scratch_edges: &mut [f32], out: &mut [f32],
    ) -> Result<(), KernelError>;

    /// Dirichlet energy ||delta s||^2. scratch_edges holds edge_floats().
    fn energy(&self, cx: &ComplexSpec<'_>, s: &[f32], scratch_edges: &mut [f32]) -> f32;

    /// out = base XOR delta_0 XOR ... XOR delta_{k-1}. out may alias base.
    fn xor_fold(
        &self, base: &[u8], deltas: &[&[u8]], out: &mut [u8],
    ) -> Result<(), KernelError>;

    /// delta_out = base XOR target, the exact-byte difference cochain.
    fn xor_encode(
        &self, base: &[u8], target: &[u8], delta_out: &mut [u8],
    ) -> Result<(), KernelError>;

    /// Morton (Z-order) key of a coordinate pair.
    fn morton_encode(&self, x: u32, y: u32) -> u64;

    /// Inverse of morton_encode.
    fn morton_decode(&self, key: u64) -> (u32, u32);

    /// Cache-blocked matrix-vector product over a row-major rows x cols matrix.
    fn matvec_tiled(
        &self, matrix: &[f32], rows: u32, cols: u32, x: &[f32], out: &mut [f32],
    ) -> Result<(), KernelError>;

    /// Compact the valid lanes of values into out. Returns the kept count.
    /// mask_bits uses little-endian 64-slot words.
    fn compact_valid(
        &self, values: &[f32], mask_bits: &[u64], out: &mut [f32],
    ) -> usize;

    /// Floats required in the Chebyshev scratch arena for this complex.
    fn chebyshev_scratch_floats(&self, cx: &ComplexSpec<'_>) -> usize;

    /// Chebyshev semi-iteration for L x = b. Returns the iteration count.
    #[allow(clippy::too_many_arguments)]
    fn chebyshev_solve(
        &self, cx: &ComplexSpec<'_>, b: &[f32], x: &mut [f32],
        max_iters: u32, tol: f32, lambda_min: f32, lambda_max: f32,
        scratch: &mut [f32],
    ) -> Result<u32, KernelError>;
}

/// Select the best adapter available in this build.
pub fn detect() -> Box<dyn KernelBackend> {
    #[cfg(sheaf_native_kernel)]
    {
        Box::new(native::NativeKernel)
    }
    #[cfg(not(sheaf_native_kernel))]
    {
        Box::new(fallback::FallbackKernel)
    }
}