//! sheaf-native - the boundary between Rust orchestration and the native tiers.
//!
//! Architecture (dependency inversion):
//!
//! ```text
//! engine / query code
//!        |  depends on the trait only
//!        v
//! kernel::KernelBackend  (port)
//!        |---------------------------|
//!        v                           v
//! kernel::native::NativeKernel   kernel::fallback::FallbackKernel
//! (C++23 library via C ABI)      (pure Rust, always available)
//! ```
//!
//! build.rs activates the native adapter when it finds zig, clang++, or g++.
//! Without a toolchain the fallback keeps the whole workspace buildable and
//! testable. Both adapters implement identical mathematics, and the parity
//! tests in tests/ compare them against an independent naive reference.

pub mod kernel;
pub mod codec;
pub mod math;
pub mod ebpf;
pub mod storage;
pub mod wal;
pub mod backends;
pub mod hypercell;
pub mod committed_records;

#[cfg(sheaf_native_kernel)]
pub mod abi;

/// True when this build links the compiled C++23 kernel library.
pub const fn native_kernel_linked() -> bool {
    cfg!(sheaf_native_kernel)
}

/// True when this build links the compiled Zig storage library.
pub const fn native_storage_linked() -> bool {
    cfg!(sheaf_native_storage)
}

/// True when this build links the compiled C11 WAL library.
pub const fn native_wal_linked() -> bool {
    cfg!(sheaf_native_wal)
}