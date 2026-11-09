// crates/sheaf-engine/src/bin/imdb_fiber_sections_bench.rs — B12 gate:
// the bundle cache keeps a PER-SECTION GENERATION table, so a changed input
// rewrites ONLY the affected section instead of the whole bundle.
//
// Rows = gate, Columns = result.
use std::path::PathBuf;

use sheaf_storage::lsp::ring_wal::crc_hw;

#[path = "shared/imdb_fiber.rs"]
mod imdb_fiber;
use imdb_fiber::{rewrite_sections, BundleReader, BundleWriter, SectionUpdate};

const SECTIONS: [&str; 4] = ["basics", "ratings", "names", "principals"];
const ROWS: usize = 20_000;

fn section_bytes(seed: u64) -> Vec<u8> {
    // synthetic but deterministic section payload
    let mut v = Vec::with_capacity(ROWS * 13);
    for i in 0..ROWS {
        v.extend_from_slice(&(seed * 1_000_003 + i as u64).to_le_bytes());
        v.extend_from_slice(&(2020i32).to_le_bytes());
        v.push((i % 251) as u8);
    }
    v
}

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join("sheaf_imdb_sections");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn main() {
    let old_path = dir().join("bundle_old.bin");
    let new_path = dir().join("bundle_new.bin");
    let _ = std::fs::remove_file(&old_path);
    let _ = std::fs::remove_file(&new_path);

    // ---- build the original bundle (each section has its own generation)
    let mut w = BundleWriter::create(old_path.to_str().unwrap()).unwrap();
    let mut gens = Vec::new();
    for (i, name) in SECTIONS.iter().enumerate() {
        let gen = 1000u64 + i as u64;
        gens.push(gen);
        w.write_section_gen(name, &section_bytes(i as u64), gen).unwrap();
    }
    w.finish().unwrap();
    let old_size = std::fs::metadata(&old_path).unwrap().len();
    println!("bundle_build: sections={} file_bytes={old_size}", SECTIONS.len());

    // ---- change ONE input: only the "ratings" section is updated
    let new_ratings = section_bytes(99);
    let updates = vec![SectionUpdate {
        name: "ratings".to_string(),
        data: new_ratings.clone(),
        gen: 2001,
    }];
    let stats = rewrite_sections(
        old_path.to_str().unwrap(),
        new_path.to_str().unwrap(),
        &updates,
    )
    .unwrap();
    let new_size = std::fs::metadata(&new_path).unwrap().len();
    println!(
        "bundle_rewrite: sections_total={} rewritten={} bytes_rewritten={} bytes_copied={} total={} file={}",
        stats.sections_total,
        stats.sections_rewritten,
        stats.bytes_rewritten,
        stats.bytes_copied,
        stats.total_bytes,
        new_size
    );

    // ---- gate 1: ONLY the changed section was re-encoded
    // (the payload bytes, not the file bytes: the header+index are
    // regenerated wholesale and are not "section" data)
    let payload_bytes: u64 = {
        let r = BundleReader::open(old_path.to_str().unwrap()).unwrap();
        r.section_names()
            .iter()
            .map(|n| r.section_meta(n).unwrap().len)
            .sum()
    };
    let only_one = stats.sections_rewritten == 1
        && stats.bytes_rewritten == new_ratings.len() as u64
        && stats.bytes_copied == payload_bytes - stats.bytes_rewritten
        && stats.bytes_rewritten + stats.bytes_copied == payload_bytes
        && stats.total_bytes == new_size;
    println!(
        "gate_bundle_only_changed_section_rewritten={}",
        if only_one { "PASS" } else { "FAIL" }
    );

    // ---- gate 2: the untouched sections are byte-identical, the changed one
    //      carries the new bytes; the generations moved as expected.
    let mut r_old = BundleReader::open(old_path.to_str().unwrap()).unwrap();
    let mut r_new = BundleReader::open(new_path.to_str().unwrap()).unwrap();
    let mut identical = true;
    let mut gens_ok = true;
    for (i, name) in SECTIONS.iter().enumerate() {
        let a = r_old.section(name).unwrap();
        let b = r_new.section(name).unwrap();
        if *name == "ratings" {
            if b != new_ratings || a == b {
                identical = false;
            }
            if r_new.section_gen(name) != Some(2001) {
                gens_ok = false;
            }
        } else {
            if a != b {
                identical = false;
            }
            if r_new.section_gen(name) != r_old.section_gen(name) || r_new.section_gen(name) != Some(gens[i]) {
                gens_ok = false;
            }
        }
    }
    println!(
        "gate_bundle_sections_intact={}",
        if identical { "PASS" } else { "FAIL" }
    );
    println!(
        "gate_bundle_per_section_generations={}",
        if gens_ok { "PASS" } else { "FAIL" }
    );

    // ---- gate 3: totals exact - the sum over all section CRCs matches a
    //      recomputation from the section bytes (no silent truncation).
    let mut expect_crcs: Vec<(String, u32)> = Vec::new();
    for (i, name) in SECTIONS.iter().enumerate() {
        let data = if *name == "ratings" { new_ratings.clone() } else { section_bytes(i as u64) };
        expect_crcs.push((name.to_string(), crc_hw(&data)));
    }
    let mut totals_ok = true;
    for (name, crc) in &expect_crcs {
        let m = r_new.section_meta(name).unwrap();
        if m.crc != *crc || m.len as usize != {
            let d = if name == "ratings" { &new_ratings } else { &section_bytes(SECTIONS.iter().position(|x| x == name).unwrap() as u64) };
            d.len()
        } {
            totals_ok = false;
        }
    }
    println!(
        "gate_bundle_totals_exact={}",
        if totals_ok { "PASS" } else { "FAIL" }
    );

    // ---- gate 4: an update for an unknown section is refused
    let bad = rewrite_sections(
        old_path.to_str().unwrap(),
        dir().join("bundle_bad.bin").to_str().unwrap(),
        &[SectionUpdate { name: "nope".to_string(), data: vec![1, 2, 3], gen: 1 }],
    );
    println!(
        "gate_bundle_unknown_section_reject={}",
        if bad.is_err() { "PASS" } else { "FAIL" }
    );

    let all = only_one && identical && gens_ok && totals_ok && bad.is_err();
    println!("IMDB_SECTIONS_BENCH: {}", if all { "PASS" } else { "FAIL" });
    let _ = std::fs::remove_file(&old_path);
    let _ = std::fs::remove_file(&new_path);
    if !all {
        std::process::exit(1);
    }
}
