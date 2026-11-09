// crates/sheaf-storage/src/bin/segment_bench.rs — B8 gate:
// the durable segment descriptor round-trips a reader exactly and rejects
// corruption (never a half-loaded segment).
//
// Rows = gate, Columns = result.
use std::path::PathBuf;
use std::time::Instant;

use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::bcsf::reader::BcsfReader;
use sheaf_storage::bcsf::segment::{open_segment, segment_meta, write_segment};
use sheaf_storage::lsp::mem_complex::MemComplex;

const N0: usize = 2_000; // 0-cells (faces)
const N2: usize = 500; // dim-2 cells (hypercells) referencing faces
const GEN: u64 = 42;
const FP: u64 = 0xDEAD_BEEF_1234_5678;

fn build() -> BcsfReader {
    let mut b = BcsfBuilder::new();
    for i in 0..N0 {
        let mut disc = [0u8; 16];
        disc[..8].copy_from_slice(&(i as u64).to_le_bytes());
        b.push(0, Vec::new(), &disc, &[i as f32, (i * 2) as f32], &[i as f32 * 0.5]);
    }
    for j in 0..N2 {
        let a = (j * 3) % N0;
        let c = (j * 3 + 1) % N0;
        let mut disc = [0u8; 16];
        disc[..8].copy_from_slice(&(N0 as u64 + j as u64).to_le_bytes());
        b.push(
            2,
            vec![a as u64, c as u64],
            &disc,
            &[j as f32, j as f32 + 1.0, j as f32 + 2.0],
            &[],
        );
    }
    b.finish()
}

fn same(a: &BcsfReader, b: &BcsfReader) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for i in 0..a.len() {
        if a.dim(i) != b.dim(i)
            || a.boundary(i) != b.boundary(i)
            || a.stalk(i) != b.stalk(i)
            || a.morph_weights(i) != b.morph_weights(i)
            || a.discrete(i) != b.discrete(i)
        {
            return false;
        }
    }
    true
}

fn main() {
    let dir = std::env::temp_dir().join("sheaf_segment_bench");
    std::fs::create_dir_all(&dir).unwrap();
    let path: PathBuf = dir.join("seg.bcsfseg");

    let src = build();
    let t0 = Instant::now();
    write_segment(&src, &path, GEN, FP).unwrap();
    let write_ms = t0.elapsed().as_secs_f64() * 1e3;
    let bytes = std::fs::metadata(&path).unwrap().len();

    let t1 = Instant::now();
    let opened = open_segment(&path).unwrap();
    let read_ms = t1.elapsed().as_secs_f64() * 1e3;

    let exact = same(&src, &opened);
    println!("segment_write: cells={} bytes={} write_ms={write_ms:.1}", src.len(), bytes);
    println!("segment_open: read_ms={read_ms:.1} cells={}", opened.len());
    println!(
        "gate_segment_exact={} (cells={} identical={})",
        if exact { "PASS" } else { "FAIL" },
        src.len(),
        exact
    );

    // metadata round-trip (generation + source fingerprint)
    let (g, f) = segment_meta(&path).unwrap();
    let meta_ok = g == GEN && f == FP;
    println!(
        "gate_segment_meta={} (gen={g} fp={f:#x})",
        if meta_ok { "PASS" } else { "FAIL" }
    );

    // negative 1: flip a byte in the payload -> CRC must reject it
    let mut raw = std::fs::read(&path).unwrap();
    let mid = raw.len() / 2;
    raw[mid] ^= 0xFF;
    let bad = dir.join("seg_corrupt.bcsfseg");
    std::fs::write(&bad, &raw).unwrap();
    let rejected = open_segment(&bad).is_err();
    println!(
        "gate_segment_corrupt_reject={} ({})",
        if rejected { "PASS" } else { "FAIL" },
        open_segment(&bad).err().unwrap_or_default()
    );

    // negative 2: truncated file -> must not half-load
    let trunc = dir.join("seg_trunc.bcsfseg");
    let cut = raw.len() / 3;
    std::fs::write(&trunc, &raw[..cut]).unwrap();
    let trunc_rejected = open_segment(&trunc).is_err();
    println!(
        "gate_segment_truncated_reject={}",
        if trunc_rejected { "PASS" } else { "FAIL" }
    );

    // negative 3: a foreign file must not be mistaken for a segment
    let alien = dir.join("alien.bcsfseg");
    std::fs::write(&alien, b"not a segment at all, just some bytes").unwrap();
    let alien_rejected = open_segment(&alien).is_err();
    println!(
        "gate_segment_alien_reject={}",
        if alien_rejected { "PASS" } else { "FAIL" }
    );

    // --- engine-level round-trip: persist a base segment and restore it
    // into a FRESH engine (the segment opens WITHOUT running the builder).
    let wal = dir.join("restore.wal");
    let _ = std::fs::remove_file(&wal);
    let mem = MemComplex::new(std::sync::Arc::new(build()), 2, &wal).unwrap();
    let seg2 = dir.join("mem_base.bcsfseg");
    mem.persist_base_segment(&seg2, GEN, FP).unwrap();
    let mem2 = MemComplex::new(
        std::sync::Arc::new(BcsfBuilder::new().finish()),
        2,
        &dir.join("restore2.wal"),
    )
    .unwrap();
    let restored = mem2.restore_base_segment(&seg2).unwrap();
    let snap_a = mem.snapshot();
    let snap_b = mem2.snapshot();
    let mut base_equal = snap_a.base.len() == snap_b.base.len();
    if base_equal {
        for i in 0..snap_a.base.len() {
            if snap_a.base.dim(i) != snap_b.base.dim(i)
                || snap_a.base.boundary(i) != snap_b.base.boundary(i)
                || snap_a.base.stalk(i) != snap_b.base.stalk(i)
                || snap_a.base.discrete(i) != snap_b.base.discrete(i)
                || snap_a.base_alive(i) != snap_b.base_alive(i)
            {
                base_equal = false;
                break;
            }
        }
    }
    println!(
        "gate_segment_restore_exact={} (cells={restored} base_identical={base_equal})",
        if base_equal { "PASS" } else { "FAIL" }
    );
    let _ = std::fs::remove_file(&wal);
    let wal2 = dir.join("restore2.wal");
    let _ = std::fs::remove_file(&wal2);
    let _ = std::fs::remove_file(&seg2);

    let all = exact && meta_ok && rejected && trunc_rejected && alien_rejected && base_equal;
    println!("SEGMENT_BENCH: {}", if all { "PASS" } else { "FAIL" });
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&bad);
    let _ = std::fs::remove_file(&trunc);
    let _ = std::fs::remove_file(&alien);
    if !all {
        std::process::exit(1);
    }
}
