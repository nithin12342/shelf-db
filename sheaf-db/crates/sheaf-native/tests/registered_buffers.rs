// registered_buffers.rs - integration test for the pre-registered buffers (MF-08).
//
// The test drives the production path: RegisteredBuffers::new allocates the
// aligned pool through the native tier, register pins it, and write_fixed
// submits one IORING_OP_WRITE_FIXED. Every case states its expected result.
//
// The Linux fixed-write case reads the file back and compares the CRC32C of
// the written bytes with the CRC32C of the bytes on disk. On a platform with
// no io_uring the paired test asserts the explicit Unsupported report. It does
// not accept a fake success.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(target_os = "linux")]
use sheaf_native::wal::crc32c::crc32c;
use sheaf_native::wal::registered::{
    platform_capability, Capability, RegBufError, RegisteredBuffers, ALIGNMENT, MAX_BUFFERS,
};

/// A unique scratch path per test. The files are removed at the end of each
/// test, so a failed run leaves at most one file behind.
fn temp_path(tag: &str) -> PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let index = SEQ.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "sheaf-regbuf-{}-{}-{}.log",
        std::process::id(),
        tag,
        index
    ))
}

/// A pool of `count` buffers of `bytes` bytes each over a scratch file.
fn pool(tag: &str, count: u32, bytes: u32) -> (RegisteredBuffers, PathBuf) {
    let path = temp_path(tag);
    let pool = RegisteredBuffers::new(&path, count, bytes).expect("the pool must allocate");
    (pool, path)
}

#[test]
fn every_buffer_is_aligned_and_the_count_matches_the_pool() {
    let (mut pool, path) = pool("align", 6, 4096);
    assert_eq!(pool.count(), 6, "the pool must report the requested count");
    assert_eq!(pool.buffer_bytes(), 4096);
    assert_eq!(pool.geometry(), (6, 4096));
    assert_eq!(pool.count(), MAX_BUFFERS.min(6));

    let mut previous: Option<usize> = None;
    for index in 0..pool.count() {
        let buffer = pool.buffer(index).expect("the index is inside the pool");
        assert_eq!(buffer.len(), 4096, "every buffer holds the full size");
        let address = buffer.as_ptr() as usize;
        assert_eq!(
            address % ALIGNMENT,
            0,
            "buffer {index} must start on a 64-byte boundary, got {address:#x}"
        );
        if let Some(last) = previous {
            assert_eq!(
                address,
                last + 4096,
                "buffer {index} must follow the previous buffer with no gap"
            );
        }
        previous = Some(address);
    }

    // An index at the count and past it are both outside the pool.
    assert_eq!(
        pool.buffer(6).err(),
        Some(RegBufError::BufferIndexOutOfRange { index: 6, count: 6 })
    );
    assert_eq!(
        pool.buffer(u32::MAX).err(),
        Some(RegBufError::BufferIndexOutOfRange { index: u32::MAX, count: 6 })
    );

    drop(pool);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_second_register_reports_already_registered() {
    let (mut pool, path) = pool("twice", 4, 4096);
    match pool.register() {
        Ok(()) => {
            assert!(pool.is_registered(), "a successful register must report the state");
            assert_eq!(
                pool.register(),
                Err(RegBufError::AlreadyRegistered),
                "the second register must be rejected"
            );
            pool.unregister().expect("unregister");
            assert!(!pool.is_registered());
        }
        Err(RegBufError::Unsupported { reason }) => {
            assert!(!reason.is_empty(), "an absent capability must name the cause");
            assert!(!pool.is_registered());
            assert_eq!(
                pool.register(),
                Err(RegBufError::Unsupported { reason }),
                "a repeat call on an unsupported platform must report the same cause"
            );
        }
        Err(other) => panic!("register failed for an unexpected reason: {other}"),
    }
    drop(pool);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_write_outside_the_pool_and_a_bad_length_are_rejected() {
    let (mut pool, path) = pool("bounds", 3, 1024);
    assert_eq!(
        pool.write_fixed(3, 1024).err(),
        Some(RegBufError::BufferIndexOutOfRange { index: 3, count: 3 })
    );
    assert_eq!(
        pool.write_fixed(0, 0).err(),
        Some(RegBufError::InvalidArgument(
            "the length is zero or above the buffer size"
        ))
    );
    assert_eq!(
        pool.write_fixed(0, 1025).err(),
        Some(RegBufError::InvalidArgument(
            "the length is zero or above the buffer size"
        ))
    );
    drop(pool);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_pool_size_that_breaks_the_alignment_is_rejected() {
    let path = temp_path("size");
    assert_eq!(
        RegisteredBuffers::new(&path, 2, 4095).err(),
        Some(RegBufError::InvalidArgument(
            "the buffer size must be a multiple of 64 bytes"
        ))
    );
    assert_eq!(
        RegisteredBuffers::new(&path, 2, 0).err(),
        Some(RegBufError::InvalidArgument("the buffer size must be above zero"))
    );
    assert_eq!(
        RegisteredBuffers::new(&path, 0, 4096).err(),
        Some(RegBufError::InvalidArgument(
            "the buffer count must be between one and 64"
        ))
    );
    assert_eq!(
        RegisteredBuffers::new(&path, MAX_BUFFERS + 1, 4096).err(),
        Some(RegBufError::InvalidArgument(
            "the buffer count must be between one and 64"
        ))
    );
    assert!(
        RegisteredBuffers::new(&path, MAX_BUFFERS, 4096).is_ok(),
        "the largest allowed pool must allocate"
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn the_platform_capability_carries_a_reason_when_it_is_absent() {
    match platform_capability() {
        Capability::IoUring => {
            assert_eq!(platform_capability().reason(), None);
        }
        Capability::Unsupported { reason } => {
            assert!(!reason.is_empty(), "an absent capability must name the cause");
            assert!(!platform_capability().is_supported());
        }
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn a_fixed_write_reports_unsupported_on_this_platform() {
    let (mut pool, path) = pool("unsupported", 2, 4096);
    let capability = pool.capability();
    let reason = capability
        .reason()
        .expect("a platform without io_uring must name the cause")
        .to_string();
    assert!(!reason.is_empty());
    assert_eq!(pool.register(), Err(RegBufError::Unsupported { reason: reason.clone() }));
    assert!(!pool.is_registered(), "a failed register must not claim the state");
    assert_eq!(
        pool.write_fixed(0, 4096),
        Err(RegBufError::Unsupported { reason }),
        "a fixed write must report the same cause, never a fake success"
    );
    drop(pool);
    let _ = std::fs::remove_file(&path);
}

#[cfg(target_os = "linux")]
#[test]
fn a_four_kib_fixed_write_lands_on_disk_with_a_matching_crc() {
    let path = temp_path("fixed");
    let mut pool = RegisteredBuffers::new(&path, 2, 4096).expect("pool");

    if !pool.capability().is_supported() {
        // The kernel has no io_uring. The reason is asserted, not ignored.
        let reason = pool.capability().reason().expect("a cause").to_string();
        assert!(!reason.is_empty());
        assert_eq!(pool.register(), Err(RegBufError::Unsupported { reason }));
        let _ = std::fs::remove_file(&path);
        return;
    }

    // Fill buffer zero with a deterministic pattern and keep the CRC.
    let payload: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
    let expected = crc32c(&payload);
    pool.buffer(0).expect("buffer 0").copy_from_slice(&payload);

    pool.register().expect("register");
    assert!(pool.is_registered());
    let written = pool.write_fixed(0, 4096).expect("fixed write");
    assert_eq!(written, 4096, "the kernel must report the full byte count");
    pool.fsync().expect("fsync");
    pool.unregister().expect("unregister");
    drop(pool);

    let on_disk = std::fs::read(&path).expect("read back");
    assert!(on_disk.len() >= 4096, "the file must hold at least one slot");
    let slot = &on_disk[..4096];
    assert_eq!(slot, payload.as_slice(), "the bytes on disk must equal the buffer");
    assert_eq!(crc32c(slot), expected, "the CRC32C read back must match");
    let _ = std::fs::remove_file(&path);
}
