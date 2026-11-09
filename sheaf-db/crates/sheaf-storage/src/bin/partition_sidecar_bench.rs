// crates/sheaf-storage/src/bin/partition_sidecar_bench.rs — B10 gate:
// the L3-domain assignment + ghost map SURVIVE process exit.
//
// Phase "write":  partition in RAM, persist the sidecar, exit.
// Phase "verify": a DIFFERENT process loads the sidecar from disk only
//                 (no re-partition) and re-checks the plan invariants.
//
// Rows = gate, Columns = result.
use std::path::PathBuf;

use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::bcsf::partition::{
    domains_fit_l3, ghosts_cover_boundaries, partition_reader, Partition,
};
use sheaf_storage::bcsf::reader::BcsfReader;

const N: usize = 4_000;
const TARGET: usize = 16 * 1024; // small target -> many domains + ghosts
const GEN: u64 = 5;
const FP: u64 = 0x0F0F_1234_5678_9ABC;

fn build_reader() -> BcsfReader {
    let mut b = BcsfBuilder::new();
    for i in 0..N {
        let mut disc = [0u8; 16];
        disc[..8].copy_from_slice(&(i as u64).to_le_bytes());
        // every cell references the PREVIOUS cell -> cross-domain references
        // (and therefore ghost replicas) appear at every domain border.
        let boundary = if i == 0 { Vec::new() } else { vec![(i - 1) as u64] };
        b.push(0, boundary, &disc, &[i as f32], &[]);
    }
    b.finish()
}

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join("sheaf_partition_sidecar");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_phase() {
    let reader = build_reader();
    let part = partition_reader(&reader, TARGET);
    let path = dir().join("partition.sidecar");
    part.write_sidecar(&path, GEN, FP).unwrap();
    let ghosts: usize = part.domains.iter().map(|d| d.ghosts.len()).sum();
    println!(
        "partition_write: domains={} ghosts={ghosts} bytes={}",
        part.domains.len(),
        std::fs::metadata(&path).unwrap().len()
    );
    println!("PARTITION_PHASE: write done");
}

fn verify_phase() {
    let path = dir().join("partition.sidecar");
    let (part, gen, fp) = Partition::read_sidecar(&path).unwrap();
    println!("partition_load: generation={gen} fingerprint={fp:#x}");

    let on_disk = std::fs::read(&path).unwrap();
    let reser = part.to_sidecar(gen, fp);
    let identical = reser == on_disk;
    println!(
        "gate_partition_reserialize_identical={} (bytes={})",
        if identical { "PASS" } else { "FAIL" },
        on_disk.len()
    );

    let reader = build_reader();
    // the plan invariants must STILL hold on the reloaded map
    let fits = domains_fit_l3(&part, TARGET);
    println!("gate_partition_domains_fit_l3={}", if fits { "PASS" } else { "FAIL" });
    let covered = ghosts_cover_boundaries(&reader, &part);
    println!(
        "gate_partition_ghosts_cover_boundaries={}",
        if covered { "PASS" } else { "FAIL" }
    );

    // the reloaded map must equal a fresh partition of the same reader
    let fresh = partition_reader(&reader, TARGET);
    let same = fresh.domains.len() == part.domains.len()
        && fresh
            .domains
            .iter()
            .zip(part.domains.iter())
            .all(|(a, b)| a.id == b.id && a.cell_range == b.cell_range && a.ghosts == b.ghosts);
    println!(
        "gate_partition_matches_fresh_plan={}",
        if same { "PASS" } else { "FAIL" }
    );

    // a corrupt sidecar must be rejected, never half-loaded
    let mut bad = on_disk.clone();
    let mid = bad.len() / 2;
    bad[mid] ^= 0xFF;
    let bad_path = dir().join("partition_corrupt.sidecar");
    std::fs::write(&bad_path, &bad).unwrap();
    let rejected = Partition::read_sidecar(&bad_path).is_err();
    println!(
        "gate_partition_corrupt_reject={} ({})",
        if rejected { "PASS" } else { "FAIL" },
        Partition::read_sidecar(&bad_path).err().unwrap_or_default()
    );
    let _ = std::fs::remove_file(&bad_path);

    let all = identical && fits && covered && same && rejected;
    println!("PARTITION_PHASE: verify {}", if all { "PASS" } else { "FAIL" });
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
            eprintln!("usage: partition_sidecar_bench [write|verify] (got '{other}')");
            std::process::exit(2);
        }
    }
}
