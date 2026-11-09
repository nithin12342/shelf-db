// crates/sheaf-engine/src/bin/fiber_lifecycle_bench.rs — N5 + N6 gates.
//
// N5: a fiber built DURING a concurrent compaction equals a fiber built on
//     a quiet box (byte-equal aggregates), and the merge exposes the epoch
//     it was frozen at.
// N6: 10,000 mixed read/write ops - the fiber's answer equals the fallback
//     scan's answer EVERY time; no deadlock, no corruption; the lag (how
//     far the fiber is behind the engine) is bounded and reported.
//
// Rows = gate, Columns = result.
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use sheaf_storage::bcsf::fiber_domain::FiberDomains;
use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::lsp::compactor::Compactor;
use sheaf_storage::lsp::fiber_epoch::{base_fingerprint, decide, Epoch, FiberDecision};
use sheaf_storage::lsp::mem_complex::MemComplex;

const BASE_CELLS: usize = 20_000;
const SHARDS: usize = 4;
const OPS: usize = 10_000;
const WRITES_EVERY: usize = 3; // 1 write per 3 ops
const DOMAIN_BYTES: usize = 1 << 20;

/// Build the fiber (and the fallback) from a snapshot: key = the cell's
/// discrete id, value = the cell index.
fn fiber_of(snap: &sheaf_storage::lsp::snapshot::Snapshot) -> FiberDomains {
    let mut entries: Vec<(u64, u32)> = Vec::with_capacity(snap.base.len());
    for i in 0..snap.base.len() {
        let id = u64::from_le_bytes(snap.base.discrete(i)[..8].try_into().unwrap());
        entries.push((id, i as u32));
    }
    FiberDomains::build(&entries, DOMAIN_BYTES)
}

/// The FALLBACK: a full scan of the same snapshot.
fn scan_lookup(snap: &sheaf_storage::lsp::snapshot::Snapshot, key: u64) -> Option<u32> {
    for i in 0..snap.base.len() {
        let id = u64::from_le_bytes(snap.base.discrete(i)[..8].try_into().unwrap());
        if id == key {
            return Some(i as u32);
        }
    }
    None
}

/// The aggregate both paths must agree on: sum of the base cells' stalk[0].
fn aggregate(snap: &sheaf_storage::lsp::snapshot::Snapshot) -> f64 {
    let mut s = 0.0f64;
    for i in 0..snap.base.len() {
        s += snap.base.stalk(i).first().copied().unwrap_or(0.0) as f64;
    }
    s
}

fn base_reader() -> sheaf_storage::bcsf::reader::BcsfReader {
    let mut b = BcsfBuilder::new();
    for i in 0..BASE_CELLS {
        let mut disc = [0u8; 16];
        disc[..8].copy_from_slice(&(i as u64 + 1).to_le_bytes());
        b.push(0, Vec::new(), &disc, &[i as f32, 1.0], &[]);
    }
    b.finish()
}

fn main() {
    let dir = std::env::temp_dir().join("sheaf_fiber_lifecycle");
    std::fs::create_dir_all(&dir).unwrap();
    let wal = dir.join("lifecycle.wal");
    let _ = std::fs::remove_file(&wal);

    eprintln!("[stage] mem created");
    let mem = Arc::new(MemComplex::new(Arc::new(base_reader()), SHARDS, &wal).unwrap());
    eprintln!("[stage] base built");

    // =====================================================================
    // N5: build during a concurrent compaction == build on a quiet box
    // =====================================================================
    // quiet box
    eprintln!("[stage] taking the quiet snapshot");
    let quiet_snap = mem.snapshot();
    eprintln!("[stage] quiet snapshot done");
    let quiet_fp = base_fingerprint(&quiet_snap.base);
    let quiet_fiber = fiber_of(&quiet_snap);
    let quiet_agg = aggregate(&quiet_snap);
    let quiet_epoch = quiet_snap.epoch(quiet_fp);
    let quiet_fiber_agg: f64 = {
        // the fiber's aggregate over its own domain entries
        let mut s = 0.0f64;
        for d in &quiet_fiber.domains {
            for (_, cell) in &d.sorted {
                s += quiet_snap.base.stalk(*cell as usize).first().copied().unwrap_or(0.0) as f64;
            }
        }
        s
    };

    // concurrent box: a compactor merges while a writer thread inserts
    eprintln!("[stage] spawning the compactor");
    let (compactor, join) = Compactor::spawn(mem.clone());
    eprintln!("[stage] compactor spawned");
    let writes = Arc::new(AtomicUsize::new(0));
    let writer_mem = mem.clone();
    let writer_writes = writes.clone();
    let writer = std::thread::spawn(move || {
        for i in 0..500u64 {
            let _ = writer_mem.insert_hypercell(
                (i % SHARDS as u64) as usize,
                &[],
                i,
                [i as f32, 0.0],
            );
            writer_writes.fetch_add(1, Ordering::Relaxed);
            std::thread::yield_now();
        }
    });

    // the fiber built while the compaction is in flight
    eprintln!("[stage] taking the busy snapshot");
    let busy_snap = mem.snapshot();
    eprintln!("[stage] busy snapshot done");
    let busy_fp = base_fingerprint(&busy_snap.base);
    let busy_fiber = fiber_of(&busy_snap);
    let busy_agg = aggregate(&busy_snap);
    let busy_epoch = busy_snap.epoch(busy_fp);
    let busy_fiber_agg: f64 = {
        let mut s = 0.0f64;
        for d in &busy_fiber.domains {
            for (_, cell) in &d.sorted {
                s += busy_snap.base.stalk(*cell as usize).first().copied().unwrap_or(0.0) as f64;
            }
        }
        s
    };
    eprintln!("[stage] merging");
    let merge_stats = compactor.merge().expect("merge");
    eprintln!("[stage] merge done");
    writer.join().unwrap();
    eprintln!("[stage] writer joined");
    // close the compactor's channel BEFORE joining: its worker loop waits on
    // recv() until the sender is dropped (joining while the sender is alive
    // would wait forever - a real hang found by this bench).
    drop(compactor);
    let _ = join.join();
    eprintln!("[stage] compactor joined");

    // the aggregates must be byte-equal (the frozen snapshot sees the same
    // data regardless of the concurrent activity)
    let quiet_bytes = quiet_agg.to_bits();
    let busy_bytes = busy_agg.to_bits();
    let same_agg = quiet_bytes == busy_bytes;
    let same_fiber_agg = quiet_fiber_agg.to_bits() == busy_fiber_agg.to_bits();
    println!(
        "n5_builds: quiet_agg={quiet_agg:.1} busy_agg={busy_agg:.1} quiet_fiber_agg={quiet_fiber_agg:.1} busy_fiber_agg={busy_fiber_agg:.1}"
    );
    println!(
        "gate_n5_concurrent_build_equals_quiet={} (aggregates_equal={same_agg} fiber_aggregates_equal={same_fiber_agg})",
        if same_agg && same_fiber_agg { "PASS" } else { "FAIL" }
    );
    println!(
        "gate_n5_frozen_epoch_exposed={} (merge_epoch gen={} fp={:#x} busy_snapshot_gen={})",
        if merge_stats.frozen_epoch.generation > 0 { "PASS" } else { "FAIL" },
        merge_stats.frozen_epoch.generation,
        merge_stats.frozen_epoch.src_fingerprint,
        busy_epoch.generation
    );
    // A fiber built BEFORE the merge must be detected as stale once the
    // merge advanced the epoch (the base changed under it).
    let post_snap = mem.snapshot();
    let post_fp = base_fingerprint(&post_snap.base);
    let post_epoch = post_snap.epoch(post_fp);
    let stale_detected = !quiet_epoch.matches(&post_epoch)
        && decide(&quiet_epoch, &post_epoch) != FiberDecision::Use;
    println!(
        "gate_n5_epoch_stamp_detects_staleness={} (pre-merge gen={} fp={:#x} vs post-merge gen={} fp={:#x} => {})",
        if stale_detected { "PASS" } else { "FAIL" },
        quiet_epoch.generation,
        quiet_epoch.src_fingerprint,
        post_epoch.generation,
        post_epoch.src_fingerprint,
        decide(&quiet_epoch, &post_epoch) != FiberDecision::Use
    );

    // =====================================================================
    // N6: 10,000 mixed read/write ops
    // =====================================================================
    let mismatches = Arc::new(AtomicUsize::new(0));
    let max_lag = Arc::new(AtomicU64::new(0));
    let reads = Arc::new(AtomicU64::new(0));
    let write_ops = Arc::new(AtomicU64::new(0));
    let m_mismatch = mismatches.clone();
    let m_lag = max_lag.clone();
    let m_reads = reads.clone();
    let m_writes = write_ops.clone();
    let t0 = Instant::now();

    eprintln!("[stage] n6 loop start");
    let mut fiber = fiber_of(&mem.snapshot());
    let mut fiber_epoch = Epoch::new(0, 0);
    let mut tx = 0u64;
    for op in 0..OPS {
        if op % WRITES_EVERY == 0 {
            // a WRITE
            let shard = (op / WRITES_EVERY) % SHARDS;
            let mut disc = [0u8; 16];
            disc[..8].copy_from_slice(&(BASE_CELLS as u64 + 10_000 + op as u64).to_le_bytes());
            let _ = mem.insert_hypercell(shard, &[], tx, [tx as f32, 1.0]);
            tx += 1;
            m_writes.fetch_add(1, Ordering::Relaxed);
            // the fiber is now behind: record the lag
            let gen = mem.generation();
            let lag = gen.saturating_sub(fiber_epoch.generation);
            m_lag.fetch_max(lag, Ordering::Relaxed);
        } else {
            // a READ: the fiber must answer exactly like the fallback scan
            let snap = mem.snapshot();
            let fp = base_fingerprint(&snap.base);
            let epoch = snap.epoch(fp);
            match decide(&fiber_epoch, &epoch) {
                FiberDecision::Use => {}
                FiberDecision::FallbackToScan(_) => {
                    // the fiber is stale for this base: rebuild it (this is
                    // the N3 path - a stale fiber NEVER answers)
                    fiber = fiber_of(&snap);
                    fiber_epoch = epoch;
                }
            }
            let key = (op as u64 % BASE_CELLS as u64) + 1;
            let a = fiber.lookup(key);
            let b = scan_lookup(&snap, key);
            if a != b {
                m_mismatch.fetch_add(1, Ordering::Relaxed);
            }
            // the aggregate must match the fallback scan too
            let fiber_agg: f64 = {
                let mut s = 0.0f64;
                for d in &fiber.domains {
                    for (_, cell) in &d.sorted {
                        s += snap.base.stalk(*cell as usize).first().copied().unwrap_or(0.0) as f64;
                    }
                }
                s
            };
            if fiber_agg.to_bits() != aggregate(&snap).to_bits() {
                m_mismatch.fetch_add(1, Ordering::Relaxed);
            }
            m_reads.fetch_add(1, Ordering::Relaxed);
        }
    }
    eprintln!("[stage] n6 loop done");
    let elapsed = t0.elapsed().as_secs_f64() * 1e3;
    let mism = mismatches.load(Ordering::Relaxed);
    let lag = max_lag.load(Ordering::Relaxed);
    println!(
        "n6_ops: ops={OPS} reads={} writes={} mismatches={mism} max_lag_generations={lag} elapsed_ms={elapsed:.0}",
        reads.load(Ordering::Relaxed),
        write_ops.load(Ordering::Relaxed)
    );
    println!(
        "gate_n6_fiber_equals_fallback={}",
        if mism == 0 { "PASS" } else { "FAIL" }
    );
    println!(
        "gate_n6_no_deadlock={} (completed {OPS} ops in {elapsed:.0} ms)",
        if elapsed < 120_000.0 { "PASS" } else { "FAIL" }
    );
    println!(
        "gate_n6_lag_bounded={} (max_lag={lag} generations)",
        if lag <= 64 { "PASS" } else { "FAIL" }
    );

    let _ = std::fs::remove_file(&wal);
    let all = same_agg
        && same_fiber_agg
        && merge_stats.frozen_epoch.generation > 0
        && stale_detected
        && mism == 0
        && elapsed < 120_000.0
        && lag <= 64;
    println!("FIBER_LIFECYCLE_BENCH: {}", if all { "PASS" } else { "FAIL" });
    if !all {
        std::process::exit(1);
    }
}
