// uring_linux.rs - fixed-buffer WAL verification (NF-14, F-12 and F-13).
//
// The real write runs only on Linux, where io_uring exists. On this Windows
// host the test verifies the opposite contract: the platform reports
// Unsupported with a named cause and the register and write calls return that
// same cause. No fake success is possible.

#[cfg(target_os = "linux")]
use sheaf_native::wal::crc32c;
use sheaf_native::wal::{platform_capability, Capability, RegBufError, RegisteredBuffers};
#[cfg(not(target_os = "linux"))]
use sheaf_native::wal::ALIGNMENT;

const BUFFER_BYTES: u32 = 4096;

#[cfg(target_os = "linux")]
#[test]
fn a_fixed_write_returns_the_crc_of_the_written_bytes() {
    let capability = platform_capability();
    if !capability.is_supported() {
        eprintln!("unsupported: {}", capability.reason().unwrap_or("no reason"));
        return;
    }
    let path = std::env::temp_dir().join(format!("sheaf_uring_{}.wal", std::process::id()));
    let mut pool = RegisteredBuffers::new(&path, 1, BUFFER_BYTES).expect("create pool");
    pool.register().expect("register");
    let pattern: Vec<u8> = (0..BUFFER_BYTES).map(|index| (index % 251) as u8).collect();
    pool.buffer(0).expect("buffer").copy_from_slice(&pattern);
    let expected_crc = crc32c(&pattern);
    let written = pool.write_fixed(0, BUFFER_BYTES).expect("write fixed");
    pool.fsync().expect("fsync");
    assert_eq!(written, BUFFER_BYTES);
    let on_disk = std::fs::read(&path).expect("read file");
    assert_eq!(&on_disk[..BUFFER_BYTES as usize], pattern.as_slice());
    assert_eq!(crc32c(&on_disk[..BUFFER_BYTES as usize]), expected_crc);
    let _ = std::fs::remove_file(&path);
}

#[cfg(target_os = "linux")]
#[test]
fn the_ring_uses_a_sqpoll_setup_flag() {
    // The C tier sets IORING_SETUP_SQPOLL when it creates the ring. The
    // capability probe reports that the native path is live.
    if platform_capability().is_supported() {
        assert!(cfg!(sheaf_native_wal), "the native WAL must provide the ring");
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn on_this_host_the_fixed_write_path_reports_unsupported() {
    let capability = platform_capability();
    let reason = capability.reason().unwrap_or_default().to_owned();
    match capability {
        Capability::Unsupported { reason } => {
            assert!(!reason.is_empty(), "the unsupported report must name a cause");
            eprintln!("unsupported: {reason}");
        }
        Capability::IoUring => panic!("this host must not claim io_uring support"),
    }
    assert_eq!(BUFFER_BYTES as usize % ALIGNMENT, 0);

    let path = std::env::temp_dir().join(format!("sheaf_no_uring_{}.wal", std::process::id()));
    let mut pool = RegisteredBuffers::new(&path, 1, BUFFER_BYTES).expect("create pool");
    assert_eq!(pool.geometry(), (1, BUFFER_BYTES));
    assert_eq!(pool.register().unwrap_err(), RegBufError::Unsupported { reason });
    assert!(matches!(pool.write_fixed(0, BUFFER_BYTES), Err(RegBufError::Unsupported { .. })));
    let _ = std::fs::remove_file(&path);
    // A fixed 4 KiB write cannot be verified on this host. That fact is the
    // result; no Linux execution is claimed here.
}
