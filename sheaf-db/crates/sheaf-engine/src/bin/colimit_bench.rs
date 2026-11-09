// crates/sheaf-engine/src/bin/colimit_bench.rs — MATH-UPGRADES Stage M2 gate
// Encodes a customer/payment fixture, appends OUT OF ORDER (2-cells before
// 0-cells) into GradedWal, seals graded, projects via colimit, and verifies:
// (1) output validates, (2) boundary slices resolve to the right faces,
// (3) stalk totals match a direct eager build exactly.
use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::lsp::colimit::{decode_cell, encode_cell, project};
use sheaf_storage::lsp::graded_log::GradedWal;

fn main() {
    let dir = std::env::temp_dir().join("sheaf_colimit_bench");
    let _ = std::fs::remove_dir_all(&dir);
    let mut g = GradedWal::create(&dir).unwrap();

    // Fixture: 3 customers (dim 0, stalk=[balance]) + 2 payments (dim 2,
    // stalk=[amount], boundary = 2 customer faces each).
    let customers: Vec<(u64, f32)> = vec![(101, 5000.0), (102, 300.0), (103, 9999.0)];
    type Boundary = Vec<(u8, u64)>;
    let payments: Vec<(u64, f32, Boundary)> = vec![
        (9001, 5000.0, vec![(0, 101), (0, 102)]),
        (9002, 9999.0, vec![(0, 102), (0, 103)]),
    ];
    // Append hypercells FIRST (out of order on purpose).
    for (id, amt, b) in &payments {
        g.append(2, &encode_cell(2, *id, b, &[*amt], &id.to_le_bytes())).unwrap();
    }
    for (id, bal) in &customers {
        g.append(0, &encode_cell(0, *id, &[], &[*bal], &id.to_le_bytes())).unwrap();
    }
    // Premature seal(2) must fail (dim 0 present but unsealed).
    let early = g.seal(2).is_err();
    println!("gate_ordered={}", if early { "PASS" } else { "FAIL" });
    assert!(g.seal(0).is_ok() && g.seal(2).is_ok());

    // Project and verify.
    let reader = project(&g).unwrap();
    assert!(reader.validate().is_ok());
    println!("gate_validate=PASS (cells={})", reader.len());

    // Boundary correctness: each payment's faces resolve to customer stalks.
    let mut ok = true;
    let mut total = 0f64;
    for cell in 0..reader.len() {
        if reader.dim(cell) == 2 {
            let b = reader.boundary(cell);
            if b.len() != 2 {
                ok = false;
            }
            for f in b {
                if reader.dim(*f as usize) != 0 {
                    ok = false;
                }
            }
            total += reader.stalk(cell)[0] as f64;
        }
    }
    // Totals must match a direct eager build.
    let mut b2 = BcsfBuilder::new();
    let db = [0u8; 16];
    let mut gid = std::collections::HashMap::new();
    for (i, (id, bal)) in customers.iter().enumerate() {
        gid.insert(*id, i);
        b2.push(0, Vec::new(), &db, &[*bal], &[]);
    }
    for (_id, amt, b) in &payments {
        let faces: Vec<u64> = b.iter().map(|(_, i)| gid[i] as u64).collect();
        b2.push(2, faces, &db, &[*amt], &[]);
    }
    let r2 = b2.finish();
    let mut total2 = 0f64;
    for cell in 0..r2.len() {
        if r2.dim(cell) == 2 {
            total2 += r2.stalk(cell)[0] as f64;
        }
    }
    let parity = (total - total2).abs() < 1e-6 && (total - 14999.0).abs() < 1e-3;
    println!("gate_boundaries={} gate_parity={} (colimit={} eager={})",
        if ok { "PASS" } else { "FAIL" }, if parity { "PASS" } else { "FAIL" }, total, total2);

    // Round-trip: every encoded frame decodes losslessly.
    let f = encode_cell(2, 7, &[(0, 1), (0, 2)], &[1.5, 2.5], &[9u8; 16]);
    let (d, id, b, s, db2) = decode_cell(&f).unwrap();
    let rt = d == 2 && id == 7 && b == vec![(0, 1), (0, 2)] && s == vec![1.5, 2.5] && db2[0] == 9;
    println!("gate_roundtrip={}", if rt { "PASS" } else { "FAIL" });

    let _ = std::fs::remove_dir_all(&dir);
    if !(early && ok && parity && rt) {
        std::process::exit(1);
    }
    println!("STAGE-M2: PASS");
}
