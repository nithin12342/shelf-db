//! native - FFI adapter that forwards the Tier 1 port to the C++23 library.
//!
//! This module compiles only when build.rs found a toolchain and linked
//! libsheaf_kernel.a (cfg sheaf_native_kernel). Every unsafe block below
//! is bounded by one rule: the pointers come from live Rust slices whose
//! lengths were validated first, and they do not escape the call.

use super::{ComplexSpec, KernelBackend, KernelError};
use crate::abi;

/// FFI adapter. Stateless, so core-pinned workers share it freely.
pub struct NativeKernel;

impl NativeKernel {
    /// Report the instruction sets the native library detected at runtime.
    pub fn cpu_features() -> u32 {
        unsafe { abi::sheaf_kernel_cpu_features() }
    }
}

// Build the borrowed C descriptor from a validated Rust spec. The returned
// value must not outlive the slices inside `cx`; every use below is scoped.
fn desc(cx: &ComplexSpec<'_>) -> abi::SheafComplexDesc {
    abi::SheafComplexDesc {
        num_vertices: cx.num_vertices,
        num_edges: cx.num_edges(),
        stalk_dim: cx.stalk_dim,
        edge_src: cx.edge_src.as_ptr(),
        edge_dst: cx.edge_dst.as_ptr(),
        r_src: cx.r_src.map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
        r_dst: cx.r_dst.map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
    }
}

fn map_err(code: i32) -> KernelError {
    match code {
        abi::SHEAF_KERNEL_ESCRATCH => KernelError::ScratchTooSmall,
        abi::SHEAF_KERNEL_ENOCONV => KernelError::NoConvergence,
        _ => KernelError::InvalidArgument,
    }
}

fn check(rc: i32) -> Result<(), KernelError> {
    if rc == abi::SHEAF_KERNEL_OK {
        Ok(())
    } else {
        Err(map_err(rc))
    }
}

impl KernelBackend for NativeKernel {
    fn name(&self) -> &'static str {
        "cpp23-native"
    }

    fn coboundary(
        &self, cx: &ComplexSpec<'_>, s: &[f32], edge_out: &mut [f32],
    ) -> Result<(), KernelError> {
        cx.validate()?;
        if s.len() != cx.vertex_floats() || edge_out.len() != cx.edge_floats() {
            return Err(KernelError::InvalidArgument);
        }
        let d = desc(cx);
        // SAFETY: d borrows cx's slices; s and edge_out lengths match the
        // descriptor; the call writes exactly edge_floats() elements.
        check(unsafe {
            abi::sheaf_coboundary_apply(&d, s.as_ptr(), edge_out.as_mut_ptr())
        })
    }

    fn coboundary_adjoint(
        &self, cx: &ComplexSpec<'_>, edge_vec: &[f32], vertex_out: &mut [f32],
    ) -> Result<(), KernelError> {
        cx.validate()?;
        if edge_vec.len() != cx.edge_floats() || vertex_out.len() != cx.vertex_floats() {
            return Err(KernelError::InvalidArgument);
        }
        let d = desc(cx);
        // SAFETY: same borrowing rule as coboundary; output is vertex-sized.
        check(unsafe {
            abi::sheaf_coboundary_adjoint(&d, edge_vec.as_ptr(), vertex_out.as_mut_ptr())
        })
    }

    fn laplacian_apply(
        &self, cx: &ComplexSpec<'_>, s: &[f32], scratch_edges: &mut [f32], out: &mut [f32],
    ) -> Result<(), KernelError> {
        cx.validate()?;
        if s.len() != cx.vertex_floats()
            || scratch_edges.len() < cx.edge_floats()
            || out.len() != cx.vertex_floats()
        {
            return Err(KernelError::InvalidArgument);
        }
        let d = desc(cx);
        // SAFETY: scratch is edge-sized, out is vertex-sized, no aliasing.
        check(unsafe {
            abi::sheaf_laplacian_apply(
                &d, s.as_ptr(), scratch_edges.as_mut_ptr(), out.as_mut_ptr(),
            )
        })
    }

    fn energy(&self, cx: &ComplexSpec<'_>, s: &[f32], scratch_edges: &mut [f32]) -> f32 {
        if cx.validate().is_err()
            || s.len() != cx.vertex_floats()
            || scratch_edges.len() < cx.edge_floats()
        {
            return -1.0;
        }
        let d = desc(cx);
        // SAFETY: buffers validated above; the library only reads s.
        unsafe { abi::sheaf_energy(&d, s.as_ptr(), scratch_edges.as_mut_ptr()) }
    }

    fn xor_fold(
        &self, base: &[u8], deltas: &[&[u8]], out: &mut [u8],
    ) -> Result<(), KernelError> {
        if out.len() != base.len() || deltas.iter().any(|d| d.len() != base.len()) {
            return Err(KernelError::InvalidArgument);
        }
        let ptrs: Vec<*const u8> = deltas.iter().map(|d| d.as_ptr()).collect();
        // SAFETY: ptrs holds live slice pointers for this call only; len is
        // uniform; out is base-sized. out may alias base, which the C side
        // documents as supported.
        check(unsafe {
            abi::sheaf_xor_delta_fold(
                base.as_ptr(), ptrs.as_ptr(), ptrs.len() as u32, base.len(),
                out.as_mut_ptr(),
            )
        })
    }

    fn xor_encode(
        &self, base: &[u8], target: &[u8], delta_out: &mut [u8],
    ) -> Result<(), KernelError> {
        if base.len() != target.len() || delta_out.len() != base.len() {
            return Err(KernelError::InvalidArgument);
        }
        // SAFETY: all three buffers share the validated length.
        check(unsafe {
            abi::sheaf_xor_delta_encode(
                base.as_ptr(), target.as_ptr(), base.len(), delta_out.as_mut_ptr(),
            )
        })
    }

    fn morton_encode(&self, x: u32, y: u32) -> u64 {
        unsafe { abi::sheaf_morton_encode(x, y) }
    }

    fn morton_decode(&self, key: u64) -> (u32, u32) {
        let (mut x, mut y) = (0u32, 0u32);
        // SAFETY: both out-pointers reference live local variables.
        unsafe { abi::sheaf_morton_decode(key, &mut x, &mut y) };
        (x, y)
    }

    fn matvec_tiled(
        &self, matrix: &[f32], rows: u32, cols: u32, x: &[f32], out: &mut [f32],
    ) -> Result<(), KernelError> {
        let (r, c) = (rows as usize, cols as usize);
        if matrix.len() != r * c || x.len() != c || out.len() != r {
            return Err(KernelError::InvalidArgument);
        }
        // SAFETY: dimensions validated against every buffer length.
        check(unsafe {
            abi::sheaf_matvec_tiled(matrix.as_ptr(), rows, cols, x.as_ptr(), out.as_mut_ptr())
        })
    }

    fn compact_valid(&self, values: &[f32], mask_bits: &[u64], out: &mut [f32]) -> usize {
        let count = values.len();
        if count == 0 || mask_bits.len() < count.div_ceil(64) || out.len() < count {
            return 0;
        }
        // SAFETY: mask covers every slot; out holds count floats; the C
        // side writes at most count elements and returns the kept count.
        unsafe {
            abi::sheaf_compact_valid(
                values.as_ptr(), mask_bits.as_ptr(), count as u32, out.as_mut_ptr(),
            ) as usize
        }
    }

    fn chebyshev_scratch_floats(&self, cx: &ComplexSpec<'_>) -> usize {
        if cx.validate().is_err() {
            return 0;
        }
        let d = desc(cx);
        unsafe { abi::sheaf_chebyshev_scratch_floats(&d) }
    }

    fn chebyshev_solve(
        &self, cx: &ComplexSpec<'_>, b: &[f32], x: &mut [f32],
        max_iters: u32, tol: f32, lambda_min: f32, lambda_max: f32,
        scratch: &mut [f32],
    ) -> Result<u32, KernelError> {
        cx.validate()?;
        let n = cx.vertex_floats();
        if b.len() != n || x.len() != n {
            return Err(KernelError::InvalidArgument);
        }
        if scratch.len() < self.chebyshev_scratch_floats(cx) {
            return Err(KernelError::ScratchTooSmall);
        }
        let d = desc(cx);
        // SAFETY: scratch size comes from the library itself; b and x are
        // vertex-sized; the descriptor borrows cx for this call only.
        let rc = unsafe {
            abi::sheaf_chebyshev_solve(
                &d, b.as_ptr(), x.as_mut_ptr(), max_iters, tol,
                lambda_min, lambda_max, scratch.as_mut_ptr(),
            )
        };
        if rc >= 0 {
            Ok(rc as u32)
        } else {
            Err(map_err(rc))
        }
    }
}