// crates/sheaf-storage/src/bin/catalog_bench.rs — B11 gate:
// a table opens BY NAME, the schema version gates every plan, and the
// catalog survives process exit.
//
// Rows = gate, Columns = result.
use std::path::PathBuf;

use sheaf_storage::bcsf::catalog::{Catalog, SegmentRef, TableMeta};

const GEN: u64 = 3;
const FP: u64 = 0x1234_5678_9ABC_DEF0;

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join("sheaf_catalog");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sample() -> Catalog {
    let t1 = TableMeta {
        name: "imdb_ratings".to_string(),
        schema_version: 4,
        column_offsets: vec![("title_id".to_string(), 0), ("rating".to_string(), 8)],
        segments: vec![
            SegmentRef { path: PathBuf::from("seg_0.bcsfseg"), generation: 1, ncells: 1000, src_fingerprint: 11 },
            SegmentRef { path: PathBuf::from("seg_1.bcsfseg"), generation: 2, ncells: 500, src_fingerprint: 12 },
        ],
    };
    let t2 = TableMeta {
        name: "imdb_titles".to_string(),
        schema_version: 7,
        column_offsets: vec![("title_id".to_string(), 0), ("year".to_string(), 8)],
        segments: vec![SegmentRef {
            path: PathBuf::from("seg_t.bcsfseg"),
            generation: 3,
            ncells: 2000,
            src_fingerprint: 13,
        }],
    };
    Catalog::new(vec![t1, t2], GEN, FP)
}

fn main() {
    let path = dir().join("catalog.sidecar");
    let cat = sample();
    cat.write_sidecar(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();

    // reload from disk (the process-restart path)
    let loaded = Catalog::read_sidecar(&path).unwrap();
    println!(
        "catalog_write: tables={} bytes={} generation={}",
        cat.tables.len(),
        bytes.len(),
        cat.generation
    );

    // 1) the catalog round-trips exactly
    let identical = loaded.to_sidecar() == bytes;
    println!(
        "gate_catalog_roundtrip={} (tables={})",
        if identical { "PASS" } else { "FAIL" },
        loaded.tables.len()
    );

    // 2) a table opens BY NAME
    let opened = loaded.open_table("imdb_ratings");
    let open_ok = match &opened {
        Ok(t) => {
            t.schema_version == 4
                && t.segments.len() == 2
                && t.segments[1].ncells == 500
                && t.column_offset("rating") == Ok(8)
        }
        Err(_) => false,
    };
    println!(
        "gate_catalog_open_by_name={} ({})",
        if open_ok { "PASS" } else { "FAIL" },
        opened.map(|t| format!("{} v{} segs={}", t.name, t.schema_version, t.segments.len()))
            .unwrap_or_else(|e| e)
    );
    println!(
        "gate_catalog_unknown_table_reject={}",
        if loaded.open_table("nope").is_err() { "PASS" } else { "FAIL" }
    );

    // 3) the schema version gates EVERY plan
    let right = loaded.plan_table("imdb_ratings", 4);
    let wrong = loaded.plan_table("imdb_ratings", 5);
    println!(
        "gate_catalog_schema_gate={} (v4 accepted={} v5 refused={})",
        if right.is_ok() && wrong.is_err() { "PASS" } else { "FAIL" },
        right.is_ok(),
        wrong.is_err()
    );
    println!("  refused: {}", wrong.as_ref().err().cloned().unwrap_or_default());

    // 4) a corrupt catalog is rejected, never half-loaded
    let mut bad = bytes.clone();
    let mid = bad.len() / 2;
    bad[mid] ^= 0xFF;
    let bad_path = dir().join("catalog_corrupt.sidecar");
    std::fs::write(&bad_path, &bad).unwrap();
    let rejected = Catalog::read_sidecar(&bad_path).is_err();
    println!(
        "gate_catalog_corrupt_reject={} ({})",
        if rejected { "PASS" } else { "FAIL" },
        Catalog::read_sidecar(&bad_path).err().unwrap_or_default()
    );
    let _ = std::fs::remove_file(&bad_path);
    let _ = std::fs::remove_file(&path);

    let all = identical && open_ok && wrong.is_err() && right.is_ok() && rejected;
    println!("CATALOG_BENCH: {}", if all { "PASS" } else { "FAIL" });
    if !all {
        std::process::exit(1);
    }
}
