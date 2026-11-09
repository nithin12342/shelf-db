// crates/sheaf-storage/src/lsp/io_uring_wal.rs — io_uring WAL writer (Linux only).
// Node H1 of IMPLEMENTATION_PLAN_HARDWARE_MAPPED_2026-10-27.md.
// On non-Linux targets this module is a no-op stub. The existing DeltaWal path
// remains the default on Windows.
#[cfg(target_os = "linux")]
mod imp {
    // Placeholder: real io_uring implementation goes here. Requires the
    // io-uring crate and a Linux kernel >= 5.1.
    pub struct IoUringWal;
    impl IoUringWal {
        pub fn create(_path: &std::path::Path) -> std::io::Result<Self> {
            Ok(Self)
        }
    }
}

#[cfg(target_os = "linux")]
pub use imp::IoUringWal;

#[cfg(not(target_os = "linux"))]
pub struct IoUringWal;

#[cfg(not(target_os = "linux"))]
impl IoUringWal {
    pub fn create(_path: &std::path::Path) -> std::io::Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "io_uring is not available on this platform",
        ))
    }
}
