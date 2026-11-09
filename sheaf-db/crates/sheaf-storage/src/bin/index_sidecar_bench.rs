// crates/sheaf-storage/src/bin/index_sidecar_bench.rs — B9 gate:
// the value->cells index (TIFI) SURVIVES process exit.
//
// Phase "write":  build the index in RAM, persist the sidecar, exit.
// Phase "verify": a DIFFERENT process loads the sidecar from disk only
//                 (no rebuild), re-serializes it and checks the bytes are
//                 identical, and checks every lookup against a full scan.
//
// Rows = gate, Columns = result.
use std::path::PathBuf;

use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::bcsf::name_lookup::NameIndex;
use sheaf_storage::bcsf::reader::BcsfReader;

const N: usize = 5_000;
const CLASS_ID: u64 = 7;
const FIELD: &str = "v";
const FIELD_OFF: usize = 8;
const GEN: u64 = 11;
const FP: u64 = 0xABCD_1234_5678_9ABC;

fn build_reader() -> BcsfReader {
    let mut b = BcsfBuilder::new();
    for i in 0..N {
        let mut disc = [0u8; 16];
        disc[..8].copy_from_slice(&CLASS_ID.to_le_bytes());
        // big-endian so the byte order is numeric order (range() needs it)
        disc[8..16].copy_from_slice(&((i % 97) as u64).to_be_bytes());
        b.push(0, Vec::new(), &disc, &[i as f32], &[]);
    }
    b.finish()
}

fn value_of(reader: &BcsfReader, cell: usize) -> u64 {
    u64::from_be_bytes(reader.discrete(cell)[8..16].try_into().unwrap())
}

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join("sheaf_index_sidecar");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_phase() {
    let reader = build_reader();
    let idx = NameIndex::build(
        &reader,
        &[(FIELD.to_string(), FIELD_OFF)],
        &[(CLASS_ID, "t".to_string())],
    );
    let path = dir().join("tifi.sidecar");
    idx.write_sidecar(&path, GEN, FP).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    println!(
        "sidecar_write: fields={} bytes={} crc={:#x}",
        idx.field_cells(FIELD).map(|c| c.len()).unwrap_or(0),
        bytes.len(),
        sheaf_storage::lsp::ring_wal::crc_hw(&bytes)
    );
    println!("SIDECAR_PHASE: write done");
}

fn verify_phase() {
    let path = dir().join("tifi.sidecar");
    let (idx, gen, fp) = NameIndex::read_sidecar(&path).unwrap();
    println!("sidecar_load: generation={gen} fingerprint={fp:#x}");

    // 1) the reloaded index re-serializes to the IDENTICAL bytes
    let reser = idx.to_sidecar(gen, fp);
    let on_disk = std::fs::read(&path).unwrap();
    let identical = reser == on_disk;
    println!(
        "gate_sidecar_reserialize_identical={} (bytes={} same={identical})",
        if identical { "PASS" } else { "FAIL" },
        on_disk.len()
    );

    // 2) parity with a full scan: every value's contiguous slice must be
    //    exactly the set of cells the scan finds.
    let reader = build_reader();
    let mut scan: std::collections::BTreeMap<u64, Vec<u32>> = std::collections::BTreeMap::new();
    for c in 0..reader.len() {
        let class = u64::from_le_bytes(reader.discrete(c)[..8].try_into().unwrap());
        if class == CLASS_ID {
            scan.entry(value_of(&reader, c)).or_default().push(c as u32);
        }
    }
    let cells = idx.field_cells(FIELD).unwrap().to_vec();
    let mut parity = true;
    let mut checked = 0usize;
    for (v, expect) in &scan {
        let key = v.to_be_bytes();
        match idx.lookup(FIELD, &key) {
            Some(r) => {
                let got: Vec<u32> = cells[r.clone()].to_vec();
                if got != *expect {
                    parity = false;
                    println!("  MISMATCH value={v} got={got:?} expect={expect:?}");
                }
                checked += 1;
            }
            None => {
                parity = false;
                println!("  MISSING value={v}");
            }
        }
    }
    let total_indexed: usize = scan.values().map(|v| v.len()).sum();
    let total_cells = cells.len();
    println!(
        "gate_sidecar_scan_parity={} (values={checked} indexed_cells={total_indexed} scan_cells={total_cells})",
        if parity && total_indexed == total_cells { "PASS" } else { "FAIL" }
    );

    // 3) range() over the reloaded index
    let r = idx.range(FIELD, 10, 20);
    let expect_ranges = scan
        .iter()
        .filter(|(v, _)| **v >= 10 && **v < 20)
        .map(|(_, cells)| cells.len())
        .sum::<usize>();
    let got_ranges: usize = r.iter().map(|rr| rr.end - rr.start).sum();
    println!(
        "gate_sidecar_range_parity={} (expected={expect_ranges} got={got_ranges})",
        if expect_ranges == got_ranges { "PASS" } else { "FAIL" }
    );

    // 4) a corrupt sidecar must be rejected, never half-loaded
    let mut bad = on_disk.clone();
    let mid = bad.len() / 2;
    bad[mid] ^= 0xFF;
    let bad_path = dir().join("tifi_corrupt.sidecar");
    std::fs::write(&bad_path, &bad).unwrap();
    let rejected = NameIndex::read_sidecar(&bad_path).is_err();
    println!(
        "gate_sidecar_corrupt_reject={} ({})",
        if rejected { "PASS" } else { "FAIL" },
        NameIndex::read_sidecar(&bad_path).err().unwrap_or_default()
    );
    let _ = std::fs::remove_file(&bad_path);

    let all = identical && parity && total_indexed == total_cells && expect_ranges == got_ranges && rejected;
    println!("SIDECAR_PHASE: verify {}", if all { "PASS" } else { "FAIL" });
    if !all {
        std::process::exit(1);
    }
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "write".to_string());
    match mode.as_str() {
        "write" => write_phase(),
        "verify" => verify_phase(),
        other => {
            eprintln!("usage: index_sidecar_bench [write|verify] (got '{other}')");
            std::process::exit(2);
        }
    }
}
