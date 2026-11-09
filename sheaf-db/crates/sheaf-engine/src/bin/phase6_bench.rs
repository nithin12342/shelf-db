// crates/sheaf-engine/src/bin/phase6_bench.rs — Phase-6 hypergraph split E2E
// METHOD-015: partition into 64MB L3 domains; ghost replicas serve
// cross-socket boundary reads locally; no dangling cross-domain refs.
use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::bcsf::partition::{domains_fit_l3, ghosts_cover_boundaries, partition_reader};
use sheaf_storage::bcsf::reader::BcsfReader;

const L3_BYTES: usize = 64 * 1024 * 1024; // 64MB

fn main() {
    // Build a BCSF with enough cells to require multiple domains.
    // Each cell: 16B discrete + 16B stalk (4xf32) + boundary avg 3*8B = ~56B.
    // 64MB / 56B ≈ 1.2M cells for one domain; use 1.5M cells → 2 domains.
    let n_cells: u64 = 1_200_000;
    let n_hyper: u64 = 300_000;
    let mut builder = BcsfBuilder::new();
    // Insert 2.5M dim-0 cells (accounts).
    for i in 0..n_cells {
        let mut db = [0u8; 16];
        db[..8].copy_from_slice(&101u64.to_le_bytes()); // class 101 = Account
        db[8..16].copy_from_slice(&i.to_le_bytes());
        let stalk = vec![0.001 * i as f32, -0.002, 0.003, 0.004];
        builder.push(0, Vec::new(), &db, &stalk, &[]);
    }
    // Insert 500k dim-2 hypercells (payments), each with 3 boundary faces.
    for h in 0..n_hyper {
        let mut db = [0u8; 16];
        db[..8].copy_from_slice(&102u64.to_le_bytes()); // class 102 = Payment
        db[8..16].copy_from_slice(&h.to_le_bytes());
        let b0 = h * 3;
        let b1 = h * 3 + 1;
        let b2 = h * 3 + 2;
        let stalk = vec![0.001 * h as f32, -0.002, 0.003, 0.004];
        builder.push(2, vec![b0, b1, b2], &db, &stalk, &[]);
    }
    let reader: BcsfReader = builder.finish();
    assert!(reader.validate().is_ok());
    // Partition into 64MB domains.
    let partition = partition_reader(&reader, L3_BYTES);
    let n_domains = partition.domains.len();
    // Gate 1: multiple domains created.
    let multi_domain = n_domains >= 2;
    // Gate 2: all domains fit within 64MB.
    let fit_l3 = domains_fit_l3(&partition, L3_BYTES);
    // Gate 3: ghost replicas cover all cross-domain boundary refs.
    let ghosts_ok = ghosts_cover_boundaries(&reader, &partition);
    // Gate 4: every cell is owned by exactly one domain.
    let mut owner_count = vec![0usize; reader.len()];
    for domain in &partition.domains {
        for cell in domain.cell_range.clone() {
            owner_count[cell] += 1;
        }
    }
    let no_overlap = owner_count.iter().all(|&c| c == 1);
    println!("domains={} fit_l3={} ghosts_ok={} no_overlap={}", n_domains, fit_l3, ghosts_ok, no_overlap);
    let pass = multi_domain && fit_l3 && ghosts_ok && no_overlap;
    println!("gate_hypergraph_split={}", if pass { "PASS" } else { "FAIL" });
    if !pass {
        std::process::exit(1);
    }
    println!("PHASE6 E2E: PASS");
}
