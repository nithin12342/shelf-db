//! abi - Rust mirror of native/include/sheaf_kernel.h.
//!
//! This module compiles only when build.rs linked the native library
//! (cfg sheaf_native_kernel). The struct layout must stay byte-identical
//! to the C header. Field order and types are part of the ABI contract.

use std::ffi::c_void;

/// Capability bits, identical to the SHEAF_CPU_* macros in the header.
pub const SHEAF_CPU_AVX512F: u32 = 0x1;
pub const SHEAF_CPU_AVX2: u32 = 0x2;
pub const SHEAF_CPU_NEON: u32 = 0x4;

/// Error codes returned by the C ABI functions.
pub const SHEAF_KERNEL_OK: i32 = 0;
pub const SHEAF_KERNEL_EINVAL: i32 = -1;
pub const SHEAF_KERNEL_ESCRATCH: i32 = -2;
pub const SHEAF_KERNEL_ENOCONV: i32 = -3;

/// Borrowed complex descriptor. The pointers reference caller slices that
/// must outlive the FFI call. native.rs constructs one per call on the stack.
#[repr(C)]
pub struct SheafComplexDesc {
    pub num_vertices: u32,
    pub num_edges: u32,
    pub stalk_dim: u32,
    pub edge_src: *const u32,
    pub edge_dst: *const u32,
    pub r_src: *const f32,
    pub r_dst: *const f32,
}

extern "C" {
    pub fn sheaf_kernel_cpu_features() -> u32;
    pub fn sheaf_kernel_abi_version() -> u32;

    pub fn sheaf_coboundary_apply(
        cx: *const SheafComplexDesc,
        s: *const f32,
        edge_out: *mut f32,
    ) -> i32;

    pub fn sheaf_coboundary_adjoint(
        cx: *const SheafComplexDesc,
        edge_vec: *const f32,
        vertex_out: *mut f32,
    ) -> i32;

    pub fn sheaf_laplacian_apply(
        cx: *const SheafComplexDesc,
        s: *const f32,
        scratch_edges: *mut f32,
        out: *mut f32,
    ) -> i32;

    pub fn sheaf_energy(
        cx: *const SheafComplexDesc,
        s: *const f32,
        scratch_edges: *mut f32,
    ) -> f32;

    pub fn sheaf_xor_delta_fold(
        base: *const u8,
        deltas: *const *const u8,
        num_deltas: u32,
        len: usize,
        out: *mut u8,
    ) -> i32;

    pub fn sheaf_xor_delta_encode(
        base: *const u8,
        target: *const u8,
        len: usize,
        delta_out: *mut u8,
    ) -> i32;

    pub fn sheaf_morton_encode(x: u32, y: u32) -> u64;
    pub fn sheaf_morton_decode(key: u64, x: *mut u32, y: *mut u32);

    pub fn sheaf_matvec_tiled(
        matrix: *const f32,
        rows: u32,
        cols: u32,
        x: *const f32,
        out: *mut f32,
    ) -> i32;

    pub fn sheaf_compact_valid(
        values: *const f32,
        mask_bits: *const u64,
        count: u32,
        out: *mut f32,
    ) -> u32;

    pub fn sheaf_chebyshev_scratch_floats(cx: *const SheafComplexDesc) -> usize;

    pub fn sheaf_chebyshev_solve(
        cx: *const SheafComplexDesc,
        b: *const f32,
        x: *mut f32,
        max_iters: u32,
        tol: f32,
        lambda_min: f32,
        lambda_max: f32,
        scratch: *mut f32,
    ) -> i32;
}

/// Opaque handle type reserved for future tiers that expose stateful objects.
#[repr(C)]
pub struct OpaqueHandle {
    _private: *mut c_void,
}