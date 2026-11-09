//! Shared storage-measurement core for the physical-limits probe.
//! Bin-local module (NOT part of the library) — lives under src/bin/shared/
//! so cargo never builds it as a target or links it into the library.
//! Extracted from physical_limits_probe.rs to satisfy SPEC-022 (≤500 lines/file).
//!
//! KEY METHODOLOGY POINT — the OS cache must be bypassed.
//! A 4 KiB random read through the normal file API is served from the
//! Windows page cache in ~3 us, which is a MEMORY copy, not a disk seek.
//! Measuring storage latency that way reports a number ~50x smaller than
//! the device's real floor. Every read here uses FILE_FLAG_NO_BUFFERING
//! so the request reaches the device.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::time::Instant;

use super::phys_mem::Rng;

/// Open a file bypassing the OS page cache, so reads hit the DEVICE and
/// not the file-system cache. Without this, a 4 KiB random read measures
/// a memcpy from the OS cache (~3 us) instead of the drive (~50-150 us),
/// which is the entire point of the measurement.
///
/// Windows: FILE_FLAG_NO_BUFFERING = 0x20000000.
pub fn open_uncached(path: &std::path::Path) -> std::io::Result<File> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_NO_BUFFERING: u32 = 0x0020_0000;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_NO_BUFFERING)
            .open(path)
    }
    #[cfg(not(windows))]
    {
        File::open(path)
    }
}

/// Storage measurements.
///
/// Returns (sequential read GB/s, random 4K p50 us, write-only 4K us, fsync us).
///
/// The write-only and fsync figures are BOTH 4 KiB payloads, so the
/// durability ratio compares like with like (an earlier version compared a
/// 1 MiB buffered write against a 4 KiB fsync — a size mismatch, not a
/// durability measurement, and it produced a false gate failure).
pub fn measure_storage() -> (f64, f64, f64, f64) {
    let dir = std::env::temp_dir().join("sheaf_physical_limits_probe");
    let _ = std::fs::create_dir_all(&dir);
    let store = dir.join("store.bin");
    let syncfile = dir.join("sync.bin");
    let size = 256usize << 20; // 256 MiB

    // --- build the store file first; it must not be truncated afterwards ---
    {
        let mut f = File::create(&store).unwrap();
        let blk = vec![7u8; 1 << 20];
        for _ in 0..size / (1 << 20) {
            f.write_all(&blk).unwrap();
        }
        f.sync_all().unwrap();
    }

    // --- 1 MiB buffered-write cost (throughput reference) ---
    {
        let f = File::create(&syncfile).unwrap();
        let blk = vec![3u8; 1 << 20];
        let mut ts: Vec<u128> = Vec::with_capacity(64);
        for _ in 0..64 {
            let t0 = Instant::now();
            (&f).write_all(&blk).unwrap();
            ts.push(t0.elapsed().as_nanos());
        }
        ts.sort_unstable();
        println!("D_write_1MiB_p50_us={:.1}", ts[32] as f64 / 1e3);
    }

    // --- 4 KiB write-only, matched in size to the fsync probe ---
    let write4k_us = {
        let f = File::create(&syncfile).unwrap();
        let blk = vec![5u8; 4096];
        let mut ts: Vec<u128> = Vec::with_capacity(2000);
        for _ in 0..2000 {
            let t0 = Instant::now();
            (&f).write_all(&blk).unwrap();
            ts.push(t0.elapsed().as_nanos());
        }
        ts.sort_unstable();
        let us = ts[1000] as f64 / 1e3;
        println!("D_write_4KiB_p50_us={:.1}", us);
        us
    };

    // --- fsync floor (4 KiB write + sync) ---
    let fsync_us = {
        let f = File::create(&syncfile).unwrap();
        let blk = vec![9u8; 4096];
        let mut ts: Vec<u128> = Vec::with_capacity(200);
        for _ in 0..200 {
            let t0 = Instant::now();
            (&f).write_all(&blk).unwrap();
            f.sync_all().unwrap();
            ts.push(t0.elapsed().as_nanos());
        }
        ts.sort_unstable();
        let us = ts[100] as f64 / 1e3;
        println!(
            "D_fsync_4KiB_p50_us={:.1} p99_us={:.1}",
            us,
            ts[198] as f64 / 1e3
        );
        us
    };

    // --- sequential read throughput, OS cache BYPASSED (device limit) ---
    let seq_gbs = {
        let mut f = open_uncached(&store).unwrap();
        let mut buf = vec![0u8; 1 << 20];
        let total = size / (1 << 20);
        let t0 = Instant::now();
        let mut read = 0usize;
        for _ in 0..total {
            if f.read_exact(&mut buf).is_err() {
                break;
            }
            read += buf.len();
        }
        let secs = t0.elapsed().as_secs_f64();
        std::hint::black_box(&buf);
        let g = read as f64 / secs / 1e9;
        println!("D_seq_read_uncached_GBps={:.2}", g);
        g
    };

    // --- 4 KiB random read latency, OS cache BYPASSED (device limit) ---
    let rand_p50 = {
        let mut f = open_uncached(&store).unwrap();
        let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
        let mut buf = vec![0u8; 4096];
        let mut ts: Vec<u128> = Vec::with_capacity(2000);
        for _ in 0..2000 {
            let off = (rng.next() % ((size / 4096) as u64 - 1)) * 4096;
            let t0 = Instant::now();
            f.seek(SeekFrom::Start(off)).unwrap();
            f.read_exact(&mut buf).unwrap();
            ts.push(t0.elapsed().as_nanos());
        }
        ts.sort_unstable();
        let p50 = ts[1000] as f64 / 1e3;
        println!(
            "D_rand4k_read_uncached_p50_us={:.1} p99_us={:.1}",
            p50,
            ts[1980] as f64 / 1e3
        );
        p50
    };

    let _ = std::fs::remove_dir_all(&dir);
    (seq_gbs, rand_p50, write4k_us, fsync_us)
}
