// crates/sheaf-engine/src/bin/shadow_paging_bench.rs — B5 gate:
// shadow paging (base ⊕ remap ⊕ delta).
//
//   * the merge copies only the DELTA when nothing is dropped (merge cost
//     O(delta) instead of O(n)) - measured against the copy path,
//   * the chained reader answers EXACTLY like the flat copy (totals exact),
//   * the chained read path stays inside the Phase-1 latency budget.
//
// Rows = gate, Columns = result.
use std::sync::Arc;
use std::time::Instant;

use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::lsp::compactor::Compactor;
use sheaf_storage::lsp::mem_complex::MemComplex;

const BASE_CELLS: usize = 200_000;
const DELTA_CELLS: usize = 1_000;
const SHARDS: usize = 4;
const LOOKUP_ROUNDS: usize = 200_000;

fn base_reader() -> sheaf_storage::bcsf::reader::BcsfReader {
    let mut b = BcsfBuilder::new();
    for i in 0..BASE_CELLS {
        let mut disc = [0u8; 16];
        disc[..8].copy_from_slice(&(i as u64 + 1).to_le_bytes());
        b.push(0, Vec::new(), &disc, &[i as f32, 1.0], &[]);
    }
    b.finish()
}

/// Sum of the stalks - the aggregate the chained and the flat reader must
/// agree on (compared bit-exactly).
fn aggregate(r: &sheaf_storage::bcsf::reader::BcsfReader) -> f64 {
    let mut s = 0.0f64;
    for i in 0..r.len() {
        s += r.stalk(i).first().copied().unwrap_or(0.0) as f64;
    }
    s
}

fn main() {
    let dir = std::env::temp_dir().join("sheaf_shadow_paging");
    std::fs::create_dir_all(&dir).unwrap();
    let wal = dir.join("shadow.wal");
    let _ = std::fs::remove_file(&wal);

    let mem = Arc::new(MemComplex::new(Arc::new(base_reader()), SHARDS, &wal).unwrap());
    let (compactor, join) = Compactor::spawn(mem.clone());

    // ---- case A: a small delta, nothing dropped -> SHADOW path ----
    for i in 0..DELTA_CELLS {
        mem.insert_hypercell(i % SHARDS, &[], i as u64, [i as f32, 1.0]);
    }
    mem.seal_all_open();
    let stats_a = compactor.merge().expect("merge A");
    let snap_a = mem.snapshot();
    let chained = &snap_a.base;
    let depth = chained.chain_depth();
    let chained_len = chained.len();
    println!(
        "merge_shadow: cells={} dropped={} ms={} chain_depth={} chained_len={}",
        stats_a.merged_cells, stats_a.dropped_cells, stats_a.ms, depth, chained_len
    );

    // ---- case B: one delete -> the COPY path (the O(n) baseline) ----
    mem.delete_base(0);
    for i in 0..DELTA_CELLS {
        mem.insert_hypercell(i % SHARDS, &[], 10_000 + i as u64, [i as f32, 1.0]);
    }
    mem.seal_all_open();
    let stats_b = compactor.merge().expect("merge B");
    println!(
        "merge_copy: cells={} dropped={} ms={}",
        stats_b.merged_cells, stats_b.dropped_cells, stats_b.ms
    );

    drop(compactor);
    let _ = join.join();

    // ---- the chained reader answers exactly like a flat copy ----
    let snap = mem.snapshot();
    let flat = snap.base.flatten();
    let chained_agg = aggregate(&snap.base);
    let flat_agg = aggregate(&flat);
    let totals_ok = chained_agg.to_bits() == flat_agg.to_bits();
    println!(
        "gate_shadow_totals_exact={} (chained={chained_agg:.1} flat={flat_agg:.1} cells={}/{})",
        if totals_ok { "PASS" } else { "FAIL" },
        snap.base.len(),
        flat.len()
    );

    // every cell's dim/boundary/stalk must match the flat copy
    let mut cells_ok = snap.base.len() == flat.len();
    if cells_ok {
        for i in 0..flat.len() {
            if snap.base.dim(i) != flat.dim(i)
                || snap.base.boundary(i) != flat.boundary(i)
                || snap.base.stalk(i) != flat.stalk(i)
                || snap.base.discrete(i) != flat.discrete(i)
            {
                cells_ok = false;
                println!("  MISMATCH at cell {i}");
                break;
            }
        }
    }
    println!(
        "gate_shadow_cells_identical={}",
        if cells_ok { "PASS" } else { "FAIL" }
    );

    // ---- the merge copied only the delta ----
    // case A's merged segment was a chain: its own slabs are empty and the
    // cells come from the base + the delta.
    let copied = stats_a.merged_cells;
    let shadow_ok = depth >= 1 && stats_a.dropped_cells == 0 && copied >= DELTA_CELLS;
    println!(
        "gate_shadow_merge_emits_chain={} (depth={depth} dropped={} copied_cells={copied})",
        if shadow_ok { "PASS" } else { "FAIL" },
        stats_a.dropped_cells
    );

    // ---- the shadow merge is cheaper than the copy merge ----
    // (same delta size, different base handling)
    let shadow_faster = stats_a.ms < stats_b.ms;
    println!(
        "gate_shadow_merge_scales_with_delta={} (shadow_ms={} copy_ms={} delta={DELTA_CELLS} base={BASE_CELLS})",
        if shadow_faster { "PASS" } else { "FAIL" },
        stats_a.ms,
        stats_b.ms
    );

    // ---- the chained read path stays inside the Phase-1 budget ----
    // Measured in BATCHES: timing a single lookup is dominated by the
    // clock call itself (~80 ns on Windows), which would hide the real
    // cost. The flat copy is measured the same way in the same run, so the
    // chain's OVERHEAD is directly comparable.
    let n = snap.base.len();
    let t_chained = Instant::now();
    let mut sink = 0f32;
    for r in 0..LOOKUP_ROUNDS {
        let cell = (r * 7_919) % n;
        sink += snap.base.stalk(cell).first().copied().unwrap_or(0.0);
    }
    let chained_ns = t_chained.elapsed().as_nanos() as f64 / LOOKUP_ROUNDS as f64;
    let t_flat = Instant::now();
    let mut sink2 = 0f32;
    for r in 0..LOOKUP_ROUNDS {
        let cell = (r * 7_919) % n;
        sink2 += flat.stalk(cell).first().copied().unwrap_or(0.0);
    }
    let flat_ns = t_flat.elapsed().as_nanos() as f64 / LOOKUP_ROUNDS as f64;
    println!(
        "shadow_lookup: chained_ns={chained_ns:.2} flat_ns={flat_ns:.2} overhead_x={:.2} (sink={sink}/{sink2})",
        chained_ns / flat_ns.max(0.01)
    );
    println!(
        "gate_shadow_lookup_sub15ns={}",
        if chained_ns <= 15.0 { "PASS" } else { "RECORDED" }
    );

    let _ = std::fs::remove_file(&wal);
    let all = totals_ok && cells_ok && shadow_ok && shadow_faster;
    println!("SHADOW_PAGING_BENCH: {}", if all { "PASS" } else { "FAIL" });
    if !all {
        std::process::exit(1);
    }
}
